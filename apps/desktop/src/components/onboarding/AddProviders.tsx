import { useQueryClient } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { useState } from "react";
import { useFeedback } from "../../hooks/useFeedback";
import { providerKeys, useProviders } from "../../queries/provider";
import { errorMessage } from "../../services/client";
import type { ProviderSummary } from "../../types";
import { FeedbackText } from "../FeedbackText";
import { LargeButton } from "../LargeButton";
import { ProviderCard } from "../ProviderCard";
import { ProviderFields } from "../provider-form/ProviderFields";
import { createProvider, updateProvider, useProviderForm } from "../provider-form/useProviderForm";
import { OnboardingActions } from "./OnboardingActions";
import { OnboardingTitle } from "./OnboardingTitle";

export function AddProviders({
  ws,
  onNext,
  onBack,
}: {
  ws: string;
  onNext: () => void;
  onBack?: () => void;
}) {
  const queryClient = useQueryClient();
  const { data: providers } = useProviders(ws);
  const [form, setForm] = useState<{ provider: ProviderSummary | null } | null>(null);

  if (form) {
    return (
      <ProviderFormView
        ws={ws}
        provider={form.provider}
        onBack={() => setForm(null)}
        onSaved={() => {
          setForm(null);
          queryClient.invalidateQueries({ queryKey: providerKeys.all(ws) });
        }}
      />
    );
  }

  const hasProviders = !!providers && providers.length > 0;

  return (
    <>
      <OnboardingTitle>Add Providers</OnboardingTitle>
      <section className="w-[32rem] flex flex-col gap-2 font-inter">
        {providers && !hasProviders && (
          <p className="text-fg-muted text-sm text-center py-2">No Providers</p>
        )}
        {providers?.map((p) => (
          <ProviderCard key={p.id} provider={p} onOpen={() => setForm({ provider: p })} />
        ))}
        <button
          type="button"
          onClick={() => setForm({ provider: null })}
          className="flex items-center justify-center gap-2 border border-border rounded-xl p-3 text-sm text-fg-muted hover:text-fg hover:border-border-focus transition-colors"
        >
          <Plus size={16} />
          Add Provider
        </button>
      </section>
      <OnboardingActions onBack={onBack} className="mt-4">
        <LargeButton onClick={onNext}>{hasProviders ? "Next" : "Skip for now"}</LargeButton>
      </OnboardingActions>
    </>
  );
}

function ProviderFormView({
  ws,
  provider,
  onBack,
  onSaved,
}: {
  ws: string;
  provider: ProviderSummary | null;
  onBack: () => void;
  onSaved: () => void;
}) {
  const { state, patch, isCreate, valid } = useProviderForm(provider);
  const [busy, setBusy] = useState(false);
  const fb = useFeedback();

  async function submit() {
    if (!valid || busy) return;
    setBusy(true);
    fb.clear();
    try {
      if (isCreate) {
        await createProvider(ws, state);
      } else {
        await updateProvider(ws, provider!.id, state);
      }
      onSaved();
    } catch (e: unknown) {
      fb.err(errorMessage(e));
      setBusy(false);
    }
  }

  return (
    <>
      <OnboardingTitle>{isCreate ? "New provider" : provider!.name}</OnboardingTitle>
      <section className="w-[32rem] font-inter border border-card-border/50 bg-card rounded-xl px-3">
        <ProviderFields state={state} onChange={patch} autoFocus variant="row" />
      </section>
      <OnboardingActions onBack={onBack} className="mt-4">
        <LargeButton onClick={submit} disabled={!valid || busy}>
          {busy ? "Saving…" : isCreate ? "Add" : "Save"}
        </LargeButton>
      </OnboardingActions>
      <FeedbackText feedback={fb.feedback} />
    </>
  );
}
