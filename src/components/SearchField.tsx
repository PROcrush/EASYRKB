import { useEffect, useRef, useState, type RefObject } from "react";
import { createPortal } from "react-dom";
import styles from "./SearchField.module.css";

export function SearchField<T extends string>({ value, onChange, scope, onScopeChange, options, label, scopeLabel, inputRef, className, menuWidth = 133 }: {
  value: string; onChange: (value: string) => void; scope: T; onScopeChange: (scope: T) => void;
  options: readonly { value: T; label: string }[]; label: string; scopeLabel: string;
  menuWidth?: number;
  inputRef?: RefObject<HTMLInputElement | null> | undefined; className?: string | undefined;
}) {
  const ownInput = useRef<HTMLInputElement>(null);
  const input = inputRef ?? ownInput;
  const root = useRef<HTMLDivElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState<{ left: number; top: number } | null>(null);
  useEffect(() => {
    if (!position) return;
    const outside = (event: MouseEvent) => {
      if (!root.current?.contains(event.target as Node) && !menu.current?.contains(event.target as Node)) setPosition(null);
    };
    const close = () => setPosition(null);
    const scroll = (event: Event) => { if (!menu.current?.contains(event.target as Node)) close(); };
    const bounds = menu.current?.getBoundingClientRect();
    if (bounds && menu.current) menu.current.style.left = `${Math.max(8, Math.min(position.left, window.innerWidth - bounds.width - 8))}px`;
    window.addEventListener("mousedown", outside);
    window.addEventListener("resize", close);
    window.addEventListener("scroll", scroll, true);
    menu.current?.querySelector<HTMLElement>('[aria-checked="true"]')?.focus();
    return () => {
      window.removeEventListener("mousedown", outside);
      window.removeEventListener("resize", close);
      window.removeEventListener("scroll", scroll, true);
    };
  }, [position]);
  return <div ref={root} className={`${styles.field} ${className ?? ""}`} role="search" data-open={!!position || undefined}>
    <button type="button" className={styles.trigger} aria-label={scopeLabel} aria-haspopup="menu" aria-expanded={!!position}
      title={options.find(option => option.value === scope)?.label}
      onClick={() => {
        const rect = root.current?.getBoundingClientRect();
        if (rect) setPosition(position ? null : { left: rect.left, top: rect.bottom + 1 });
      }}><span className={styles.glass} /><span className={styles.arrow} /></button>
    <input ref={input} type="search" value={value} onChange={event => onChange(event.target.value)}
      aria-label={label} placeholder={label} autoComplete="off" spellCheck={false}
      onKeyDown={event => { if (event.key === "Escape") { onChange(""); setPosition(null); } event.stopPropagation(); }} />
    {position ? createPortal(<div ref={menu} role="menu" aria-label={scopeLabel} className={styles.menu}
      style={{ left: position.left, top: position.top, minWidth: menuWidth, maxHeight: `calc(100vh - ${position.top + 8}px)` }}
      onKeyDown={event => {
        const items = Array.from(menu.current?.querySelectorAll<HTMLButtonElement>("button") ?? []);
        const index = items.indexOf(document.activeElement as HTMLButtonElement);
        if (event.key === "Escape" || event.key === "Tab") { setPosition(null); input.current?.focus(); }
        else if (event.key === "ArrowDown") items[(index + 1) % items.length]?.focus();
        else if (event.key === "ArrowUp") items[(index + items.length - 1) % items.length]?.focus();
        else if (event.key === "Home") items[0]?.focus();
        else if (event.key === "End") items.at(-1)?.focus();
        else return;
        event.preventDefault(); event.stopPropagation();
      }}>
      {options.map(option => <button key={option.value} type="button" role="menuitemradio" aria-checked={scope === option.value}
        onClick={() => { onScopeChange(option.value); setPosition(null); input.current?.focus(); }}>
        <span aria-hidden className={styles.tick}>{scope === option.value ? "✓" : ""}</span>{option.label}
      </button>)}
    </div>, document.body) : null}
  </div>;
}
