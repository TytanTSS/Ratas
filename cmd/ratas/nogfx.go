//go:build nogfx

package main

import (
	"errors"

	"ratas/internal/client"
	"ratas/internal/config"
	"ratas/internal/i18n"
)

// Built with -tags nogfx (e.g. for a headless server): no window support.
const gfxAvailable = false

func runGfx(*config.Config, []string, client.StartOptions) error {
	return errors.New(i18n.T("графический режим не включён в эту сборку (собрано с -tags nogfx)"))
}
