/**
 * Keyboard: rekordbox's Export key map, grouped as its pane groups it.
 * Capture docs/screenshots 9.48.54 PM.
 *
 * Every row is rekordbox's — the ten groups and the commands in each, with
 * the key the Export preset binds (`keymap.ts`). A row this app answers is
 * drawn live and its key can be changed: click the badge, press the new
 * chord, and it is stored with the preferences and read by every key
 * handler through `shortcuts.ts`. Escape leaves the key as it was;
 * backspace takes it away. A chord another row already has moves to the
 * row that took it. One this app does not answer is greyed, the way
 * rekordbox greys what a selection cannot do, so the map reads the same
 * here as there. The menu's accelerators are the shell's and stay as they
 * are. Reset puts the preset back.
 */
import { useEffect, useMemo, useState } from "react";

import { TwistyIcon } from "@/components/icons";
import { KEYMAP, KEYMAP_GROUPS, keyForPlatform, type KeymapGroup } from "@/lib/keymap";
import {
  BINDINGS, chordFromEvent, describeChord, detectPlatform, sameChord, type KeyChord, type KeyOverrides,
} from "@/lib/shortcuts";
import { usePreferencesContext } from "@/store/usePreferences";
import styles from "./Preferences.module.css";
import { Button } from "./controls";

interface Row {
  id: string;
  label: string;
  /** The key as printed, or null for a row with none. */
  key: string | null;
  /** Whether this app answers it. */
  built: boolean;
  /** The binding whose key this row can change, when it can. */
  bindingId: string | null;
  /** Whether the key is the person's own rather than the preset's. */
  changed: boolean;
}

/** The chord a binding has now: the person's, or the preset's. */
function chordOf(id: string, fallback: KeyChord, overrides: KeyOverrides): KeyChord {
  return overrides[id] ?? fallback;
}

/** The pane's rows for a group: rekordbox's, then this app's own. */
export function rowsFor(group: KeymapGroup, mac: boolean, overrides: KeyOverrides = {}): Row[] {
  const platform = { mac };
  const byCommand = new Map(BINDINGS.filter((b) => b.command !== undefined).map((b) => [b.command, b]));
  const rows: Row[] = KEYMAP[group].map((row) => {
    const binding = byCommand.get(row.id);
    if (binding?.action === undefined) {
      return {
        id: row.id,
        label: row.label,
        key: row.key === null ? null : keyForPlatform(row.key, mac),
        built: binding !== undefined,
        bindingId: null,
        changed: false,
      };
    }
    const chord = chordOf(binding.id, binding.chord, overrides);
    return {
      id: row.id,
      label: row.label,
      key: chord.key === "" ? null : describeChord(chord, platform),
      built: true,
      bindingId: binding.id,
      changed: binding.id in overrides,
    };
  });
  for (const binding of BINDINGS) {
    if (binding.command !== undefined || binding.pane !== group || binding.alias) continue;
    const chord = chordOf(binding.id, binding.chord, overrides);
    rows.push({
      id: `ours:${binding.id}`,
      label: binding.label,
      key: chord.key === "" ? null : describeChord(chord, platform),
      built: true,
      bindingId: binding.action === undefined ? null : binding.id,
      changed: binding.id in overrides,
    });
  }
  return rows;
}

/**
 * The stored keys after one binding takes a chord: the binding gets it, a
 * preset key put back is forgotten rather than stored, and any other
 * binding that had the chord loses it, so a key does two things nowhere.
 */
export function assignChord(overrides: KeyOverrides, id: string, chord: KeyChord): KeyOverrides {
  const next: Record<string, KeyChord> = { ...overrides };
  const target = BINDINGS.find((b) => b.id === id);
  if (!target) return overrides;
  for (const binding of BINDINGS) {
    if (binding.id === id || binding.action === undefined) continue;
    const held = chordOf(binding.id, binding.chord, overrides);
    if (chord.key !== "" && sameChord(held, chord)) next[binding.id] = { key: "" };
  }
  if (sameChord(target.chord, chord)) delete next[id];
  else next[id] = chord;
  return next;
}

export function KeyboardPane() {
  const platform = useMemo(detectPlatform, []);
  const { preferences, update, reset } = usePreferencesContext();
  const overrides = preferences.keyboard.overrides;
  const [open, setOpen] = useState<Set<KeymapGroup>>(() => new Set());
  /** The binding waiting for its new key. */
  const [listening, setListening] = useState<string | null>(null);

  // The next chord goes to the row that asked, before any handler in the
  // window can act on it: the key the person is about to bind must not
  // also do what it does now.
  useEffect(() => {
    if (listening === null) return undefined;
    const onKey = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (event.key === "Escape") {
        setListening(null);
        return;
      }
      const chord = event.key === "Backspace" || event.key === "Delete"
        ? { key: "" }
        : chordFromEvent(event, platform);
      if (chord === null) return;
      update("keyboard", { overrides: assignChord(overrides, listening, chord) });
      setListening(null);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [listening, overrides, platform, update]);

  const changed = Object.keys(overrides).length > 0;
  return (
    <section className={`${styles.section} ${styles.sectionFill}`} aria-label="Keyboard">
      <div className={styles.actions} data-spaced="">
        <Button onClick={() => reset("keyboard")} disabled={!changed}>Reset to the preset</Button>
      </div>
      <div className={styles.keys}>
        {KEYMAP_GROUPS.map((group) => {
          const shown = open.has(group);
          const rows = shown ? rowsFor(group, platform.mac, overrides) : [];
          return (
            <div key={group}>
              <button
                type="button"
                className={styles.keyGroup}
                aria-expanded={shown}
                onClick={() =>
                  setOpen((current) => {
                    const next = new Set(current);
                    if (next.has(group)) next.delete(group);
                    else next.add(group);
                    return next;
                  })
                }
              >
                <TwistyIcon />
                {group}
              </button>
              {shown && rows.length === 0 ? (
                <div className={styles.keyRow} data-dim="">
                  <span className={styles.keyName}>Nothing is bound here in the Export preset.</span>
                </div>
              ) : null}
              {rows.map((row) => (
                <div
                  key={row.id}
                  className={styles.keyRow}
                  data-dim={row.built ? undefined : ""}
                  title={row.built ? undefined : "rekordbox has this; it is not built here"}
                >
                  <span className={styles.keyName}>{row.label}</span>
                  {row.bindingId !== null ? (
                    <button
                      type="button"
                      className={styles.keyBadge}
                      data-listening={listening === row.bindingId ? "" : undefined}
                      data-changed={row.changed ? "" : undefined}
                      aria-label={`${row.label} key`}
                      title={listening === row.bindingId
                        ? "Press the new key; Escape keeps this one, Backspace takes it away"
                        : "Click to change the key"}
                      onClick={() => setListening((current) => (current === row.bindingId ? null : row.bindingId))}
                    >
                      {listening === row.bindingId ? "…" : row.key ?? "+"}
                    </button>
                  ) : row.key !== null ? (
                    <span className={styles.keyBadge}>{row.key}</span>
                  ) : null}
                </div>
              ))}
            </div>
          );
        })}
      </div>
    </section>
  );
}
