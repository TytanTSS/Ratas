package llm

import (
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/anthropics/anthropic-sdk-go/option"
)

// fakeAPI answers /v1/messages with a canned structured reply and records requests.
func fakeAPI(t *testing.T, reply any) (*httptest.Server, *[]map[string]any) {
	var mu sync.Mutex
	var reqs []map[string]any
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if !strings.HasSuffix(r.URL.Path, "/v1/messages") {
			http.NotFound(w, r)
			return
		}
		body, _ := io.ReadAll(r.Body)
		var m map[string]any
		json.Unmarshal(body, &m)
		m["_beta"] = r.Header.Get("anthropic-beta")
		m["_key"] = r.Header.Get("x-api-key")
		mu.Lock()
		reqs = append(reqs, m)
		mu.Unlock()
		text, _ := json.Marshal(reply)
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]any{
			"id": "msg_test", "type": "message", "role": "assistant", "model": m["model"],
			"content":     []any{map[string]any{"type": "text", "text": string(text)}},
			"stop_reason": "end_turn",
			"usage":       map[string]any{"input_tokens": 100, "output_tokens": 20},
		})
	}))
	t.Cleanup(srv.Close)
	return srv, &reqs
}

func TestNPCTalk(t *testing.T) {
	api, reqs := fakeAPI(t, NPCReply{Say: "Здравствуй, путник!", Action: "offer_quest", QuestMonster: "wolf", QuestCount: 4})
	b := New("sk-test", "", option.WithBaseURL(api.URL))
	if !b.Enabled() || b.Model() != DefaultModel {
		t.Fatalf("brain not configured: %v %q", b.Enabled(), b.Model())
	}
	done := make(chan NPCReply, 1)
	b.NPCTalk(NPCRequest{
		NPCName: "Ждан", Role: "Староста", Persona: "мудрый", Village: "Ольховка", World: "Ратас",
		PlayerName: "Игрок", PlayerLevel: 2, PlayerClass: "Воин", Message: "Есть работа?",
		CanGiveQuest: true, Monsters: []Option{{"wolf", "Волк"}},
		History: []Turn{{"player", "Привет"}, {"npc", "Здравствуй"}},
	}, func(r NPCReply, err error) {
		if err != nil {
			t.Error(err)
		}
		done <- r
	})
	select {
	case r := <-done:
		if r.Action != "offer_quest" || r.QuestMonster != "wolf" || r.Say == "" {
			t.Fatalf("unexpected reply %+v", r)
		}
	case <-time.After(10 * time.Second):
		t.Fatal("no reply")
	}
	req := (*reqs)[0]
	if req["model"] != DefaultModel || req["_key"] != "sk-test" {
		t.Fatalf("bad model/key: %v %v", req["model"], req["_key"])
	}
	oc, _ := req["output_config"].(map[string]any)
	if oc == nil || oc["effort"] != "low" || oc["format"] == nil {
		t.Fatalf("output_config missing: %v", req["output_config"])
	}
	if req["fallbacks"] != "default" || !strings.Contains(req["_beta"].(string), "server-side-fallback-2026-07-01") {
		t.Fatalf("fallbacks not set: %v %v", req["fallbacks"], req["_beta"])
	}
	msgs := req["messages"].([]any)
	content := msgs[0].(map[string]any)["content"].([]any)[0].(map[string]any)["text"].(string)
	for _, want := range []string{"Ждан", "Есть работа?", "wolf: Волк", "conversation_so_far"} {
		if !strings.Contains(content, want) {
			t.Errorf("prompt lacks %q", want)
		}
	}
}

func TestTacticAndModelOptions(t *testing.T) {
	api, reqs := fakeAPI(t, TacticReply{Tactic: "call_allies", Say: "Ко мне, братья!"})
	b := New("sk-test", "claude-haiku-4-5", option.WithBaseURL(api.URL))
	done := make(chan TacticReply, 1)
	if !b.Tactic(TacticRequest{Name: "Шаман", HPPct: 40, Enemy: "Игрок", Distance: 3}, func(r TacticReply, err error) {
		if err != nil {
			t.Error(err)
		}
		done <- r
	}) {
		t.Fatal("tactic request dropped")
	}
	select {
	case r := <-done:
		if r.Tactic != "call_allies" {
			t.Fatalf("bad tactic %+v", r)
		}
	case <-time.After(10 * time.Second):
		t.Fatal("no reply")
	}
	req := (*reqs)[0]
	oc := req["output_config"].(map[string]any)
	if _, has := oc["effort"]; has {
		t.Fatal("effort must not be sent to haiku")
	}
	if _, has := req["fallbacks"]; has {
		t.Fatal("fallbacks must not be sent to haiku")
	}
}

func TestNoCredentials(t *testing.T) {
	t.Setenv("ANTHROPIC_API_KEY", "")
	t.Setenv("ANTHROPIC_AUTH_TOKEN", "")
	var b *Brain = New("", "")
	if b != nil || b.Enabled() {
		t.Fatal("brain without credentials must be nil and disabled")
	}
}
