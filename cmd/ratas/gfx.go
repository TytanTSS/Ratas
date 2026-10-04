//go:build !nogfx

package main

import (
	"ratas/internal/client"
	"ratas/internal/config"
	"ratas/internal/gfx"
)

const gfxAvailable = true

func runGfx(cfg *config.Config, mods []string, opts client.StartOptions) error {
	return gfx.Run(cfg, mods, opts)
}
