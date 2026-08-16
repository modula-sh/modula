import { Trash2 } from "lucide-react";
import type { RuleComparison, RuleModel, RuleRow } from "../lib/rules";
import { optLabel, RULE_KEYS, RULE_OPERATORS, ruleKeyFor } from "../lib/rulesSchema";
import { Button } from "./Button";
import { DropdownSelect } from "./DropdownMenu";
import { TextInput } from "./TextInput";

// Token/block builder for agent rules. Rows are OR-ed; comparisons within a row
// are AND-ed. Controlled: all edits flow through `onChange(nextModel)`.
//
// Raw-only rows (a source expression the parser couldn't reduce to comparisons)
// render read-only with a notice and are preserved untouched in `value`.

const CUSTOM = "__custom__";

// Bordered square for the per-row +/delete buttons. 1.625rem = 26px matches the
// adjacent non-padded field dropdowns (text-xs + py-1 + 1px border).
const TOKEN_BTN =
  "inline-flex items-center justify-center h-[1.625rem] w-[1.625rem] border border-border rounded leading-none hover:bg-surface-2 hover:border-border-focus/20";

function defaultValueFor(path: string): string {
  const key = ruleKeyFor(path);
  // enum and bool both seed from their first option (bool → "true").
  return key?.valueKind === "enum" || key?.valueKind === "bool" ? (key.values?.[0] ?? "") : "";
}

function defaultComparison(): RuleComparison {
  const path = RULE_KEYS[0].path;
  return { key: path, op: "==", value: defaultValueFor(path) };
}

export function RuleBuilder({
  value,
  onChange,
}: {
  value: RuleModel;
  onChange: (next: RuleModel) => void;
}) {
  function setRow(index: number, next: RuleRow) {
    onChange(value.map((row, i) => (i === index ? next : row)));
  }
  function removeRow(index: number) {
    onChange(value.filter((_, i) => i !== index));
  }

  return (
    <div className="space-y-2">
      {value.length === 0 && (
        <div className="text-[11px] text-fg-subtle italic">
          No rules. The agent only triggers manually.
        </div>
      )}
      {value.map((row, ri) => (
        <div key={ri} className="space-y-2">
          {ri > 0 && (
            <div className="flex items-center gap-2 text-[10px] uppercase tracking-wide text-fg-subtle">
              <span className="h-px flex-1 bg-border" />
              or
              <span className="h-px flex-1 bg-border" />
            </div>
          )}
          <Row row={row} onChange={(next) => setRow(ri, next)} onRemove={() => removeRow(ri)} />
        </div>
      ))}
      <Button onClick={() => onChange([...value, { comparisons: [defaultComparison()] }])}>
        + add row
      </Button>
    </div>
  );
}

function Row({
  row,
  onChange,
  onRemove,
}: {
  row: RuleRow;
  onChange: (next: RuleRow) => void;
  onRemove: () => void;
}) {
  if (row.raw != null) {
    return (
      <div className="flex items-start gap-2 border border-border rounded p-2">
        <div className="flex-1 min-w-0 space-y-1">
          <code className="block text-xs font-mono text-fg break-all">{row.raw}</code>
          <div className="text-[10px] text-fg-subtle">
            Can’t represent in builder. Edit in Raw mode.
          </div>
        </div>
        <Button tone="link" onClick={onRemove}>
          ×
        </Button>
      </div>
    );
  }

  const { comparisons } = row;
  function setComparison(index: number, next: RuleComparison) {
    onChange({ comparisons: comparisons.map((c, i) => (i === index ? next : c)) });
  }
  function removeComparison(index: number) {
    const next = comparisons.filter((_, i) => i !== index);
    if (next.length === 0) onRemove();
    else onChange({ comparisons: next });
  }

  return (
    <div className="flex items-center gap-2">
      <div className="flex flex-1 flex-wrap items-center gap-x-1.5 gap-y-2">
        {comparisons.map((c, ci) => {
          const isLast = ci === comparisons.length - 1;
          // One condition = one flex item, so it never wraps mid-condition; the
          // trailing "and" rides on its end, making that gap the only wrap point.
          return (
            <span key={ci} className="inline-flex items-center gap-1.5">
              <Comparison
                comparison={c}
                onChange={(next) => setComparison(ci, next)}
                onRemove={comparisons.length > 1 ? () => removeComparison(ci) : undefined}
              />
              {isLast ? (
                <Button
                  tone="link"
                  className={TOKEN_BTN}
                  title="add condition"
                  onClick={() => onChange({ comparisons: [...comparisons, defaultComparison()] })}
                >
                  +
                </Button>
              ) : (
                <span className="text-[10px] uppercase tracking-wide text-fg-subtle">and</span>
              )}
            </span>
          );
        })}
      </div>
      {/* Row delete sits flush right and stays vertically centred against the
          whole (possibly multi-line) condition block via the outer items-center. */}
      <Button tone="link" className={TOKEN_BTN} title="remove row" onClick={onRemove}>
        <Trash2 size={12} />
      </Button>
    </div>
  );
}

function Comparison({
  comparison,
  onChange,
  onRemove,
}: {
  comparison: RuleComparison;
  onChange: (next: RuleComparison) => void;
  onRemove?: () => void;
}) {
  const knownKey = ruleKeyFor(comparison.key);
  return (
    <span className="inline-flex items-center gap-1">
      <DropdownSelect
        variant="field"
        className="min-w-[8rem]"
        value={comparison.key}
        onChange={(v) => onChange({ ...comparison, key: v, value: defaultValueFor(v) })}
        options={[
          ...(knownKey ? [] : [{ value: comparison.key, label: optLabel(comparison.key) }]),
          ...RULE_KEYS.map((k) => ({ value: k.path, label: optLabel(k.label) })),
        ]}
      />
      <DropdownSelect
        variant="field"
        className="min-w-[5.5rem]"
        value={comparison.op}
        onChange={(v) => onChange({ ...comparison, op: v as RuleComparison["op"] })}
        options={RULE_OPERATORS.map((op) => ({ value: op.value, label: optLabel(op.label) }))}
      />
      <ValueToken comparison={comparison} onChange={(v) => onChange({ ...comparison, value: v })} />
      {onRemove && (
        <Button tone="link" className={TOKEN_BTN} title="remove condition" onClick={onRemove}>
          <Trash2 size={12} />
        </Button>
      )}
    </span>
  );
}

function ValueToken({
  comparison,
  onChange,
}: {
  comparison: RuleComparison;
  onChange: (value: string) => void;
}) {
  const key = ruleKeyFor(comparison.key);
  // enum and bool both render as a dropdown; only enum offers the custom escape
  // (bool is constrained to its two literals).
  const enumValues =
    key?.valueKind === "enum" || key?.valueKind === "bool" ? (key.values ?? []) : [];
  const allowCustom = key?.valueKind === "enum";

  if (enumValues.length === 0) {
    return (
      <TextInput
        value={comparison.value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="value"
        mono
        className="min-w-[9rem]"
      />
    );
  }

  const isCustom = allowCustom && !enumValues.includes(comparison.value);
  return (
    <span className="inline-flex items-center gap-1">
      <DropdownSelect
        variant="field"
        className="min-w-[9rem]"
        value={isCustom ? CUSTOM : comparison.value}
        onChange={(v) => onChange(v === CUSTOM ? "" : v)}
        options={[
          ...enumValues.map((v) => ({ value: v, label: optLabel(v) })),
          ...(allowCustom ? [{ value: CUSTOM, label: "custom…" }] : []),
        ]}
      />
      {isCustom && (
        <TextInput
          value={comparison.value}
          onChange={(e) => onChange(e.target.value)}
          placeholder="custom value"
          mono
          className="min-w-[8rem]"
        />
      )}
    </span>
  );
}
