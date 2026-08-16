import { BackButton } from "../BackButton";

export function OnboardingActions({
  onBack,
  children,
  className = "",
}: {
  onBack?: () => void;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div className={`flex items-center gap-3 ${className}`.trim()}>
      {onBack && <BackButton onClick={onBack}>← Back</BackButton>}
      {children}
    </div>
  );
}
