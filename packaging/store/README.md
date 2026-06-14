# Microsoft Store listing images

**Store display images** — uploaded manually in Partner Center (Store listing →
*Store logos*). They override the logos taken from the MSIX package when the Store
shows the listing to Windows 10/11 customers. All PNG, 1:1 (square), well under
the 5 MB limit, with transparency.

| File | Size | Partner Center slot |
| --- | --- | --- |
| `StoreLogo-300x300.png` | 300×300 | 1:1 App tile icon |
| `StoreLogo-150x150.png` | 150×150 | 1:1 |
| `StoreLogo-71x71.png` | 71×71 | 1:1 |

Generated from the app icon (`assets/polaris256.png`) with ImageMagick. To
regenerate:

```sh
for sz in 300 150 71; do
  magick assets/polaris256.png -background none -alpha on -filter Lanczos \
    -resize ${sz}x${sz} -strip "PNG32:packaging/store/StoreLogo-${sz}x${sz}.png"
done
```
