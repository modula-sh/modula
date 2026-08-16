import { useEffect, useState } from "react";
import { AgentIdenticon } from "../AgentIdenticon";
import { LargeButton } from "../LargeButton";
import { OnboardingActions } from "./OnboardingActions";

export function Landing({ onNext, onBack }: { onNext: () => void; onBack?: () => void }) {
  const [seed, setSeed] = useState(() => Math.random().toString(36).slice(2, 8));
  useEffect(() => {
    const timer = setInterval(() => setSeed(Math.random().toString(36).slice(2, 8)), 3000);
    return () => clearInterval(timer);
  }, []);
  return (
    <>
      <AgentIdenticon id={seed} size={80} />
      <span className="text-fg text-3xl font-mono uppercase tracking-[0.25em]">MODULA</span>
      <OnboardingActions onBack={onBack} className="mt-8">
        <LargeButton onClick={onNext}>Get started</LargeButton>
      </OnboardingActions>
    </>
  );
}
