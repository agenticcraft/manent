Linux desktop files for the RPM (decision-10).

- `ai.agenticcraft.manent.desktop` is the launcher entry. Its name is the app id the XDG
  portals know Manent by (global shortcuts need it), so it is installed under that name.
- Tauri's bundler always writes `Manent.desktop` too; `hidden.desktop.hbs` is its template,
  hidden (`NoDisplay=true`) so the launcher shows Manent once.
