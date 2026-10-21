# Vendored KaTeX

KaTeX **0.19.0**, MIT licensed (see `LICENSE`). JavaScript, CSS and WOFF2 fonts
are copied unchanged from the official npm release:

https://registry.npmjs.org/katex/-/katex-0.19.0.tgz

Archive SHA-512 (base64):
`v6Tznz3zJ7u3niRCoDTsumM2+HA2XXcCu+WAacCeHD2z3p9A9Ks987o5FzfTGBN0e8A0vjEgIDvLbICrXpdw/Q==`

Only the browser runtime, stylesheet and WOFF2 fonts are bundled. No npm/Node
runtime, CDN or build-time download is required. WebKit supports WOFF2, so the
stylesheet's older WOFF/TTF fallbacks are not bundled. To update, verify the npm
archive integrity, replace these files and fonts, retain the license, update
this record and run the Markdown GTK tests (including math and CSP checks).
