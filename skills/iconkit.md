---
name: iconkit
description: Search and download Material Symbols icons ([fonts.google.com/icons](https://fonts.google.com/icons)) from the command l
---

# iconkit

Search and download Material Symbols icons ([fonts.google.com/icons](https://fonts.google.com/icons)) from the command line: one catalog, cached locally, searched offline — then the SVG you wanted, or a ready-to-use Android VectorDrawable.

## When to use

- Reach for `iconkit` when its dedicated purpose matches the task (see description above).
- Prefer it over hand-rolled adb/shell commands for the features it wraps.

## Common commands

```bash
iconkit search alarm                      # best matches first
iconkit search clock --category Actions   # filter by category
iconkit search timer --tag countdown      # filter by tag
iconkit search clock --json               # machine-readable hits
iconkit categories                        # the category vocabulary
iconkit info alarm                        # tags, sizes, direct SVG URL

iconkit get alarm                         # alarm.svg
iconkit get alarm --vd                    # ic_alarm.xml (VectorDrawable)
iconkit get alarm 13mp --vd -o app/src/main/res/drawable
iconkit get settings --style rounded --style sharp   # one file per style
iconkit get alarm --size 48 --prefix "" -o icons     # 48 px, raw name
iconkit --refresh search alarm            # re-download the catalog
```

```bash
$ iconkit info alarm
alarm — Material Symbols Outlined
  categories  Actions
  tags        access, adjust, alarm, alert, appointment, bell, calendar, circle, clock, control +20
  popularity  4882
  sizes px    20, 24, 40, 48
  svg         https://fonts.gstatic.com/s/i/short-term/release/materialsymbolsoutlined/alarm/default/24px.svg
```

```bash
$ iconkit get alarm --vd
wrote ./ic_alarm.xml
```

With several `--style`s, file names carry the style
(`ic_alarm_outlined.xml`, `ic_alarm_sharp.svg`, …).

## Notes

- Run `iconkit --help` for the full option list.
- Device-touching commands need `adb` on PATH and an authorized device unless noted.
