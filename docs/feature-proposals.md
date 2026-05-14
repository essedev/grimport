# Feature proposals

Design sketches for features that are on the [ROADMAP](ROADMAP.md) but not yet implemented. Each one is self-contained and can be picked up independently. Shipped features are not tracked here - check [CHANGELOG.md](../CHANGELOG.md) for what's already in.

---

## Project tags and colors

**Priority**: low. Cosmetic UX improvement.

### What it does
Lets you associate each project with a color (and optionally an emoji/icon) to recognize it at a glance in the sidebar and the popover.

### UX
- "New project" and "Edit project" modals: color picker (predefined palette of 8-12 colors consistent with the dark theme).
- Sidebar: colored dot next to the project name.
- Menubar popover: same, in mini version.
- Active-ports badge inherits the project color.
- Optional: emoji picker for the project icon.

### DB schema
```sql
ALTER TABLE projects ADD COLUMN color TEXT;
ALTER TABLE projects ADD COLUMN icon TEXT;  -- emoji or lucide icon name
```

### Predefined palette
Colors that work on the Portsage dark theme with good contrast: amber (default), red, orange, yellow, green, cyan, blue, purple, pink, grey.

### Implementation
- SQLite migration to add the columns.
- Update the `create_project`, `update_project` Tauri commands + the matching socket methods.
- UI: reusable `ColorPicker` primitive.

---

## System notifications

**Priority**: low. A differentiator vs competitors (none have native notifications).

### What it does
Sends native macOS notifications on relevant events, configurable in settings.

### Notifiable events
- **Port collision**: a registered port is taken by an unexpected process.
- **Zombie port**: a registered port is active but the process does not match the configured service.
- **Range exhausted**: the global base_port + range_size has few free ranges left.
- **MCP reserve**: Claude reserved a new range (optional, can be noisy).
- **Process killed**: feedback on a kill via CLI/MCP for asynchronous confirmation.

### UX
- Settings > "Notifications": toggle for each event type.
- Defaults: collision and zombie port on, the rest off.
- Click on the notification opens the panel of the affected project.

### Implementation
- `tauri-plugin-notification` for native notifications.
- Listener in the port scanner polling: compares current vs previous state, detects events.
- Storage of recent events (last 50) in a SQLite table for a future timeline.

### macOS permissions
- On first launch, request notification permissions.
- Graceful fallback if the user denies.

---

## i18n and language switcher

**Priority**: medium. The app currently ships with English strings hardcoded. A proper i18n setup unlocks Italian (and any future language) without code changes.

### What it does
Replaces hardcoded UI strings with translation keys, adds a language switcher in settings, and persists the chosen language in the DB. Italian is the first additional locale (the project owner is Italian-speaking).

### UX
- Settings > "Language": dropdown with available languages (English, Italian).
- On change, the entire UI re-renders in the new language without restart.
- On first launch, default language is detected from the macOS system locale (`sys-locale` Rust crate); falls back to English if unsupported.
- Persisted across restarts.

### Library and stack
- `react-i18next` + `i18next` - de facto standard, supports lazy loading and pluralization.
- Locale files in `src/i18n/locales/{en,it}.json` with semantic keys (e.g. `settings.title`, `project.add.cta`), never positional keys.
- Language stored in the SQLite `config` table (`key = "language"`).
- Loaded by the Rust backend at startup and exposed to the frontend via a Tauri command (so React can boot already in the right language without flicker).

### Implementation
1. Install `i18next` and `react-i18next`.
2. Create `src/i18n/index.ts` with init logic.
3. Audit all `.tsx` files for hardcoded strings and extract them into `en.json`.
4. Create `it.json` with the original Italian translations preserved from before the English migration.
5. Replace strings in components with the `t()` hook.
6. Add the language dropdown to `SettingsPanel`.
7. Wire `i18n.changeLanguage(lang)` plus DB persistence on change.
8. On first launch, detect system locale via a Rust command and pass it to the frontend as the default.

### Tricky bits
- **Pluralization**: "1 active port" vs "5 active ports". Use `i18next` plural rules with the `count` parameter.
- **Numbers and dates**: use `Intl.NumberFormat` and `Intl.DateTimeFormat` with the current locale.
- **Backend errors**: the Rust backend should return error codes, not strings; the frontend translates them.
- **MCP server output**: stays in English (it is read by Claude, not by the user).
- **SKILL.md**: stays in English (it is a prompt for Claude).
- **Tooltips and aria-labels**: must be included in the audit, easy to forget.
- **Title bar and tray menu items**: handled in Rust; expose a Tauri command so the Rust side can fetch translated labels from the frontend or load the JSON directly.

### Languages to ship
- English (default).
- Italian (priority, the owner uses Italian daily).
- Future: any language can be added by dropping a new JSON file in `locales/`.

---

## Suggested implementation order

1. **i18n and language switcher** - reaches Italian-speaking users and any future locale.
2. **Notifications** - added value, differentiator.
3. **Tags and colors** - polish, last step.
