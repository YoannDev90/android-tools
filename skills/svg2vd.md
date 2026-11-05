---
name: svg2vd
description: Convert an SVG into an Android VectorDrawable XML. Pure conversion — no device, no Android SDK, no Android Studio: the f
---

# svg2vd

Convert an SVG into an Android VectorDrawable XML. Pure conversion — no device, no Android SDK, no Android Studio: the first fully pure crate of this workspace.

## When to use

- Reach for `svg2vd` when its dedicated purpose matches the task (see description above).
- Prefer it over hand-rolled adb/shell commands for the features it wraps.

## Common commands

```bash
svg2vd icon.svg                    # → icon.xml next to it
svg2vd icon.svg -o app/src/main/res/drawable/icon.xml
svg2vd - -o -                      # stdin → stdout, pipe-friendly
```

```bash
$ svg2vd triangle.svg -o -
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="24dp"
    android:height="24dp"
    android:viewportWidth="24"
    android:viewportHeight="24">
    <path
        android:pathData="M12 2 L2 22 L22 22 L12 2 Z"
        android:fillColor="#FFFF0000"/>
</vector>
```

## Notes

- Run `svg2vd --help` for the full option list.
- Device-touching commands need `adb` on PATH and an authorized device unless noted.
