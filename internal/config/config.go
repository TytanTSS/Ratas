// Package config stores user settings and resolves data directories.
package config

import (
	"encoding/json"
	"os"
	"path/filepath"
)

type Config struct {
	Name       string `json:"name"`
	Port       int    `json:"port"`
	LastServer string `json:"last_server"`
	APIKey     string `json:"api_key,omitempty"`
	Model      string `json:"model"`
	AIEnabled  bool   `json:"ai_enabled"`
	ASCIIOnly  bool   `json:"ascii_only"`
}

// Home is the data directory: $RATAS_HOME or ~/.ratas.
func Home() string {
	if h := os.Getenv("RATAS_HOME"); h != "" {
		return h
	}
	if h, err := os.UserHomeDir(); err == nil {
		return filepath.Join(h, ".ratas")
	}
	return ".ratas"
}

func SavesDir() string { return filepath.Join(Home(), "saves") }
func ModsDir() string  { return filepath.Join(Home(), "mods") }
func path() string     { return filepath.Join(Home(), "config.json") }

func Default() *Config {
	name := os.Getenv("USER")
	if name == "" {
		name = "Странник"
	}
	return &Config{Name: name, Port: 7777, Model: "claude-opus-5-5", AIEnabled: true, LastServer: "127.0.0.1:7777"}
}

func Load() *Config {
	c := Default()
	data, err := os.ReadFile(path())
	if err == nil {
		json.Unmarshal(data, c)
	}
	if c.Port == 0 {
		c.Port = 7777
	}
	return c
}

// Save writes the config with owner-only permissions (it may hold an API key).
func (c *Config) Save() error {
	if err := os.MkdirAll(Home(), 0o700); err != nil {
		return err
	}
	data, _ := json.MarshalIndent(c, "", "  ")
	return os.WriteFile(path(), data, 0o600)
}
