package gfx

import (
	"fmt"
	"image"
	"image/png"
	"os"
	"path/filepath"
	"time"

	"github.com/hajimehoshi/ebiten/v2"

	"ratas/internal/config"
)

// captureImage copies the current frame (halved on HiDPI screens).
func captureImage(screen *ebiten.Image, scale float64) image.Image {
	b := screen.Bounds()
	pix := make([]byte, 4*b.Dx()*b.Dy())
	screen.ReadPixels(pix)
	src := &image.RGBA{Pix: pix, Stride: 4 * b.Dx(), Rect: image.Rect(0, 0, b.Dx(), b.Dy())}
	var out image.Image = src
	if scale >= 2 {
		half := image.NewRGBA(image.Rect(0, 0, b.Dx()/2, b.Dy()/2))
		for y := 0; y < b.Dy()/2; y++ {
			for x := 0; x < b.Dx()/2; x++ {
				half.SetRGBA(x, y, src.RGBAAt(x*2, y*2))
			}
		}
		out = half
	}
	return out
}

// writePNG saves an image to a file.
func writePNG(out image.Image, path string) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	f, err := os.Create(path)
	if err != nil {
		return err
	}
	defer f.Close()
	return png.Encode(f, out)
}

func screenshotPath() string {
	return filepath.Join(config.Home(), "screenshots", fmt.Sprintf("ratas-%s.png", time.Now().Format("2006-01-02-150405")))
}
