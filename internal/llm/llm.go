// Package llm connects NPCs and elite enemies to Claude via the Anthropic API.
//
// The game never blocks on the network: every request runs in its own
// goroutine and reports back through a callback, while the built-in AI keeps
// playing. Replies use structured outputs (a JSON schema), so the model can
// only choose from actions the game knows how to validate and execute.
package llm

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"strings"
	"sync"
	"time"

	"github.com/anthropics/anthropic-sdk-go"
	"github.com/anthropics/anthropic-sdk-go/option"
	"github.com/anthropics/anthropic-sdk-go/shared/constant"
)

const DefaultModel = "claude-opus-5-5"

type Turn struct {
	Who  string // "player" or "npc"
	Text string
}

type Option struct {
	Key  string
	Name string
}

type NPCRequest struct {
	NPCName      string
	Role         string
	Persona      string
	Village      string
	World        string
	TimeOfDay    string
	PlayerName   string
	PlayerClass  string
	PlayerLevel  int
	PlayerQuests []string
	Facts        []string
	History      []Turn
	Message      string
	Trader       bool
	CanGiveQuest bool
	Gifts        []Option // items the NPC may give
	Monsters     []Option // monsters valid for quests
	Purse        int
	Region       string   // the land around, with its danger
	Mood         string   // what worries or pleases the NPC right now
	PlayerDeeds  []string // what the hero is known for
	TimesMet     int      // earlier conversations with this hero
	OwnQuest     string   // a unique character's own quest and its state
}

type NPCReply struct {
	Say          string `json:"say"`
	Action       string `json:"action"`
	Item         string `json:"item"`
	Gold         int    `json:"gold"`
	QuestMonster string `json:"quest_monster"`
	QuestCount   int    `json:"quest_count"`
}

type TacticRequest struct {
	Name        string
	Persona     string
	Boss        bool
	HPPct       int
	Allies      []string
	Enemy       string
	EnemyClass  string
	EnemyLevel  int
	EnemyHPPct  int
	Distance    int
	AbilityName string
	Events      []string
}

type TacticReply struct {
	Tactic string `json:"tactic"`
	Say    string `json:"say"`
}

var Tactics = []string{"aggressive", "defensive", "flank", "retreat", "call_allies", "use_ability"}

var NPCActions = []string{"none", "give_gold", "give_item", "heal", "offer_quest", "trade", "hostile", "end"}

// Brain is the connection to Claude. A nil *Brain is valid and disabled.
type Brain struct {
	client    anthropic.Client
	model     string
	talkSem   chan struct{}
	tacticSem chan struct{}

	mu        sync.Mutex
	calls     []time.Time
	maxPerMin int
	lastErr   string
	disabled  bool
}

// HasCredentials reports whether an API key is available from config or environment.
func HasCredentials(apiKey string) bool {
	return apiKey != "" || os.Getenv("ANTHROPIC_API_KEY") != "" || os.Getenv("ANTHROPIC_AUTH_TOKEN") != ""
}

// New creates a Brain. It returns nil when no credentials are available.
func New(apiKey, model string, opts ...option.RequestOption) *Brain {
	if !HasCredentials(apiKey) {
		return nil
	}
	if model == "" {
		model = DefaultModel
	}
	all := []option.RequestOption{option.WithMaxRetries(1)}
	if apiKey != "" {
		all = append(all, option.WithAPIKey(apiKey))
	}
	all = append(all, opts...)
	return &Brain{
		client:    anthropic.NewClient(all...),
		model:     model,
		talkSem:   make(chan struct{}, 3),
		tacticSem: make(chan struct{}, 2),
		maxPerMin: 40,
	}
}

func (b *Brain) Enabled() bool {
	if b == nil {
		return false
	}
	b.mu.Lock()
	defer b.mu.Unlock()
	return !b.disabled
}

func (b *Brain) Model() string {
	if b == nil {
		return ""
	}
	return b.model
}

// LastError returns the most recent API error (for the status line).
func (b *Brain) LastError() string {
	if b == nil {
		return ""
	}
	b.mu.Lock()
	defer b.mu.Unlock()
	return b.lastErr
}

func (b *Brain) allow() bool {
	b.mu.Lock()
	defer b.mu.Unlock()
	if b.disabled {
		return false
	}
	now := time.Now()
	keep := b.calls[:0]
	for _, t := range b.calls {
		if now.Sub(t) < time.Minute {
			keep = append(keep, t)
		}
	}
	b.calls = keep
	if len(b.calls) >= b.maxPerMin {
		return false
	}
	b.calls = append(b.calls, now)
	return true
}

func (b *Brain) fail(err error) {
	b.mu.Lock()
	defer b.mu.Unlock()
	b.lastErr = err.Error()
	var apierr *anthropic.Error
	if errors.As(err, &apierr) && (apierr.StatusCode == 401 || apierr.StatusCode == 403) {
		// a bad key will not fix itself; stop spamming the API
		b.disabled = true
	}
}

// NPCTalk asks Claude for an NPC's reply. done is called from another goroutine.
func (b *Brain) NPCTalk(req NPCRequest, done func(NPCReply, error)) {
	go func() {
		if !b.allow() {
			done(NPCReply{}, errors.New("лимит запросов к ИИ, попробуйте позже"))
			return
		}
		b.talkSem <- struct{}{}
		defer func() { <-b.talkSem }()
		ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
		defer cancel()
		var out NPCReply
		err := b.call(ctx, npcSystem, npcPrompt(req), npcSchema(), 4096, &out)
		if err != nil {
			b.fail(err)
		}
		done(out, err)
	}()
}

// Tactic asks Claude to pick a combat tactic. It returns false if the request
// was dropped (busy or rate-limited); then done is never called.
func (b *Brain) Tactic(req TacticRequest, done func(TacticReply, error)) bool {
	select {
	case b.tacticSem <- struct{}{}:
	default:
		return false
	}
	if !b.allow() {
		<-b.tacticSem
		return false
	}
	go func() {
		defer func() { <-b.tacticSem }()
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		defer cancel()
		var out TacticReply
		err := b.call(ctx, tacticSystem, tacticPrompt(req), tacticSchema(), 2048, &out)
		if err != nil {
			b.fail(err)
		}
		done(out, err)
	}()
	return true
}

// supportsEffort: the effort parameter errors on Haiku 4.5 and older models.
func (b *Brain) supportsEffort() bool {
	m := b.model
	return !strings.Contains(m, "haiku") && !strings.Contains(m, "-4-5") && !strings.Contains(m, "3-")
}

// supportsFallback: server-side refusal fallbacks for the models that run
// safety classifiers (Claude API only).
func (b *Brain) supportsFallback() bool {
	switch b.model {
	case "claude-opus-5-5", "claude-opus-5", "claude-fable-5-1", "claude-sonnet-5-5":
		return true
	}
	return false
}

func (b *Brain) call(ctx context.Context, system, user string, schema map[string]any, maxTokens int64, out any) error {
	params := anthropic.BetaMessageNewParams{
		Model:     anthropic.Model(b.model),
		MaxTokens: maxTokens,
		System:    []anthropic.BetaTextBlockParam{{Text: system}},
		Messages: []anthropic.BetaMessageParam{
			anthropic.NewBetaUserMessage(anthropic.NewBetaTextBlock(user)),
		},
		OutputConfig: anthropic.BetaOutputConfigParam{
			Format: anthropic.BetaJSONOutputFormatParam{Schema: schema},
		},
	}
	// game replies must be fast: keep reasoning short
	if b.supportsEffort() {
		params.OutputConfig.Effort = anthropic.BetaOutputConfigEffortLow
	}
	if b.supportsFallback() {
		params.Betas = append(params.Betas, anthropic.AnthropicBetaServerSideFallback2026_07_01)
		params.Fallbacks = anthropic.BetaFallbacksParamUnion{OfDefault: constant.ValueOf[constant.Default]()}
	}
	resp, err := b.client.Beta.Messages.New(ctx, params)
	if err != nil {
		var apierr *anthropic.Error
		if errors.As(err, &apierr) {
			switch apierr.StatusCode {
			case 401, 403:
				return fmt.Errorf("ключ API отклонён (%d)", apierr.StatusCode)
			case 404:
				return fmt.Errorf("модель %q не найдена", b.model)
			case 429:
				return fmt.Errorf("превышен лимит API (429)")
			default:
				if apierr.StatusCode >= 500 {
					return fmt.Errorf("сервис ИИ временно недоступен (%d)", apierr.StatusCode)
				}
				return fmt.Errorf("ошибка API %d", apierr.StatusCode)
			}
		}
		return fmt.Errorf("сеть: %w", err)
	}
	if resp.StopReason == anthropic.BetaStopReasonRefusal {
		return errors.New("модель отказалась отвечать")
	}
	if resp.StopReason == anthropic.BetaStopReasonMaxTokens {
		return errors.New("ответ ИИ обрезан")
	}
	var text strings.Builder
	for _, block := range resp.Content {
		if t, ok := block.AsAny().(anthropic.BetaTextBlock); ok {
			text.WriteString(t.Text)
		}
	}
	if err := json.Unmarshal([]byte(text.String()), out); err != nil {
		return fmt.Errorf("неверный JSON от ИИ: %w", err)
	}
	return nil
}
