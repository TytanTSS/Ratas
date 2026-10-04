// Command ratas is a real-time ASCII fantasy RPG engine for the terminal.
package main

import (
	"flag"
	"fmt"
	"log"
	"math/rand/v2"
	"os"
	"os/signal"
	"path/filepath"
	"strings"
	"syscall"

	"ratas/internal/client"
	"ratas/internal/config"
	"ratas/internal/content"
	"ratas/internal/game"
	"ratas/internal/i18n"
	"ratas/internal/llm"
	"ratas/internal/server"
)

// version is set at release build time: -ldflags "-X main.version=v1.2.3".
var version = "dev"

func main() {
	// the language is needed before the flags describe themselves
	cfg := config.Load()
	i18n.SetLang(langArg(os.Args[1:], cfg.Language))
	T := i18n.T
	var (
		join      = flag.String("join", "", T("подключиться к миру по адресу host:port"))
		host      = flag.Bool("host", false, T("открыть мир для сети (вместе с -new или -load)"))
		newGame   = flag.Bool("new", false, T("сразу начать новый мир"))
		load      = flag.String("load", "", T("загрузить сохранение по имени слота"))
		seed      = flag.Int64("seed", 0, T("зерно генерации мира"))
		name      = flag.String("name", "", T("имя героя"))
		class     = flag.String("class", "", T("класс героя: warrior, rogue, mage, priest"))
		noPvP     = flag.Bool("nopvp", false, T("новый мир без боя между игроками"))
		dedicated = flag.Bool("server", false, T("выделенный сервер без интерфейса"))
		window    = flag.Bool("gfx", false, T("графический режим в отдельном окне"))
		port      = flag.Int("port", 0, T("порт сервера (по умолчанию из настроек, 7777)"))
		mods      = flag.String("mods", "", T("каталог модов (по умолчанию ~/.ratas/mods)"))
		lang      = flag.String("lang", "", T("язык: ru или en (по умолчанию из настроек)"))
		showVer   = flag.Bool("version", false, T("показать версию и выйти"))
	)
	flag.Usage = func() {
		fmt.Fprintf(os.Stderr, "%s\n", T("Ратас — ASCII RPG в терминале.\n\nИспользование:\n  ratas                 главное меню в терминале\n  ratas -gfx            графический режим в отдельном окне\n  ratas -new -host      новый мир, открытый для друзей\n  ratas -join IP:7777   присоединиться к другу\n  ratas -server -seed 1 выделенный сервер\n\nФлаги:"))
		flag.PrintDefaults()
	}
	flag.Parse()
	if *lang != "" {
		cfg.Language = i18n.Normalize(*lang)
		i18n.SetLang(cfg.Language)
	}
	if !*window {
		ensureConsole()
	}
	if *showVer {
		fmt.Println("ratas", version)
		return
	}

	if *port != 0 {
		cfg.Port = *port
	}
	modsDir := *mods
	if modsDir == "" {
		modsDir = config.ModsDir()
	}
	db, loaded, err := content.LoadDefault(modsDir)
	if err != nil {
		fmt.Fprintln(os.Stderr, i18n.T("Ошибка контента:"), err)
		os.Exit(1)
	}
	content.Use(db)

	if *dedicated {
		runDedicated(cfg, *seed, *load, loaded, *noPvP)
		return
	}
	opts := client.StartOptions{Join: *join, Host: *host, Load: *load, New: *newGame, Seed: *seed, Name: *name, Class: *class, NoPvP: *noPvP}
	if *window {
		if err := runGfx(cfg, loaded, opts); err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}
		return
	}
	if err := client.Run(cfg, loaded, opts); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func runDedicated(cfg *config.Config, seed int64, slot string, mods []string, noPvP bool) {
	log.SetFlags(log.Ltime)
	if len(mods) > 0 {
		log.Print(i18n.Tf("моды: %s", strings.Join(mods, ", ")))
	}
	var brain *llm.Brain
	if cfg.AIEnabled {
		brain = llm.New(cfg.APIKey, cfg.Model)
	}
	if brain != nil {
		log.Print(i18n.Tf("ИИ-персонажи: %s", brain.Model()))
	} else {
		log.Print(i18n.T("ИИ-персонажи выключены (нет ключа или отключено в настройках)"))
	}
	var g *game.Game
	var err error
	if slot != "" {
		g, err = game.Load(filepath.Join(config.SavesDir(), slot+".sav"), brain)
		if err != nil {
			log.Fatal(i18n.Tf("загрузка: %v", err))
		}
	} else {
		if seed == 0 {
			seed = rand.Int64N(1_000_000)
		}
		g = game.New(seed, brain)
		g.PvP = !noPvP
		slot = fmt.Sprintf("server-%d", seed)
	}
	srv := server.New(g)
	srv.Logf = log.Printf
	srv.SavePath = filepath.Join(config.SavesDir(), slot+".sav")
	go srv.Run()
	if err := srv.Listen(fmt.Sprintf(":%d", cfg.Port)); err != nil {
		log.Fatalf("listen: %v", err)
	}
	log.Print(i18n.Tf("мир «%s» (зерно %d) открыт на порту %d; адреса: %s", g.WorldName, g.Seed, cfg.Port, strings.Join(server.LANAddresses(), ", ")))
	log.Print(i18n.Tf("сохранение: %s (каждые 3 минуты и при остановке), Ctrl+C — остановить", srv.SavePath))
	sig := make(chan os.Signal, 1)
	signal.Notify(sig, os.Interrupt, syscall.SIGTERM)
	<-sig
	srv.Call(func(g *game.Game) {
		if err := g.Save(srv.SavePath); err != nil {
			log.Print(i18n.Tf("сохранение: %v", err))
		} else {
			log.Print(i18n.T("мир сохранён"))
		}
	})
	srv.Stop()
}

// langArg finds -lang in the arguments before the flags are parsed.
func langArg(args []string, def string) string {
	for i, a := range args {
		a = strings.TrimLeft(a, "-")
		if v, ok := strings.CutPrefix(a, "lang="); ok {
			return v
		}
		if a == "lang" && i+1 < len(args) {
			return args[i+1]
		}
	}
	return def
}
