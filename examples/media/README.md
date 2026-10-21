# Media fixtures

These small fixtures were created for Ravel; no third-party media is included.
`ravel.svg` is a simple local illustration. The audio is a quiet 440 Hz tone and
the silent video is an animated test pattern, both two seconds long.

Reproduce with FFmpeg (no network access):

```sh
ffmpeg -f lavfi -i 'sine=frequency=440:duration=2' -af volume=0.2 -c:a libvorbis tone.ogg
ffmpeg -f lavfi -i 'testsrc2=size=320x180:rate=12:duration=2' -c:v libvpx -b:v 100k -an demo.webm
```
