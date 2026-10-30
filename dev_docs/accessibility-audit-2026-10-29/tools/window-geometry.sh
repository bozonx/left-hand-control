#!/usr/bin/env bash
# Run through run-isolated.sh with LHC_SCALE; KWin logs "LHCGEO {...}" lines with the window and work-area geometry (journal).
set -u
export LHC_SOCKET=lhc-geometry.sock
out=$LHC_AUDIT_OUT
name=$(kscreen-doctor -j | python3 -c 'import json,sys; print(next(o["name"] for o in json.load(sys.stdin)["outputs"] if o["enabled"]))')
kscreen-doctor "output.$name.scale.$LHC_SCALE" >/dev/null
sleep 0.5
app=$LHC_AUDIT_ROOT/target/debug/left-hand-control
"$app" > "$out/app.log" 2>&1 &
sleep 4
"$app" show settings
sleep 4
js=$out/geometry.js
cat > "$js" <<'JS'
console.info("LHCGEO start " + workspace.windowList().length);
for (const w of workspace.windowList()) {
  if (!String(w.resourceClass).includes("left-hand")) continue;
  const f = w.frameGeometry, c = w.clientGeometry, a = workspace.clientArea(KWin.PlacementArea, w);
  console.info("LHCGEO " + JSON.stringify({cls: String(w.resourceClass), frame: [f.x, f.y, f.width, f.height], client: [c.x, c.y, c.width, c.height], area: [a.x, a.y, a.width, a.height], min: [w.minSize.width, w.minSize.height]}));
}
JS
raw=$(gdbus call --session --dest org.kde.KWin --object-path /Scripting --method org.kde.kwin.Scripting.loadScript "$js" lhcgeo 2>&1)
echo "load: $raw" >> "$out/probe.log"
id=$(printf '%s' "$raw" | tr -dc '0-9')
gdbus call --session --dest org.kde.KWin --object-path "/Scripting/Script$id" --method org.kde.kwin.Script.run >> "$out/probe.log" 2>&1
sleep 1
"$app" quit
sleep 1
