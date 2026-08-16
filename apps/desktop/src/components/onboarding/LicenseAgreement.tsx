import { useState } from "react";
import { LargeButton } from "../LargeButton";
import { openUrl } from "../openUrl";
import { OnboardingActions } from "./OnboardingActions";
import { OnboardingTitle } from "./OnboardingTitle";

const LICENSE_ID = "Elastic-2.0";
const LICENSE_URL = "https://github.com/modula-sh/modula/blob/main/LICENSE";
const TERMS_URL = "https://github.com/modula-sh/modula/blob/main/TERMS.md";
const linkClass = "text-sm text-fg underline underline-offset-4 hover:text-fg-muted";

export type LicenseAcceptance = {
  license: string;
  licenseUrl: string;
  termsUrl: string;
  acceptedAt: string;
  appVersion: string;
};

export function LicenseAgreement({
  onAccept,
  onBack,
}: {
  onAccept: (acceptance: LicenseAcceptance) => void;
  onBack?: () => void;
}) {
  const [checked, setChecked] = useState(false);

  return (
    <>
      <OnboardingTitle>License</OnboardingTitle>
      <p className="text-fg-muted text-sm font-inter -mt-2 max-w-md text-center">
        By using Modula, you agree to the terms of the license agreement and the Terms of Use.
      </p>
      <div className="flex flex-col items-center gap-5 font-inter">
        <div className="flex items-center gap-4">
          <button type="button" onClick={() => openUrl(LICENSE_URL)} className={linkClass}>
            View license
          </button>
          <button type="button" onClick={() => openUrl(TERMS_URL)} className={linkClass}>
            View terms
          </button>
        </div>
        <label className="flex items-center gap-2 text-sm text-fg">
          <input type="checkbox" checked={checked} onChange={(e) => setChecked(e.target.checked)} />
          <span>I have read and agree to the license and Terms of Use</span>
        </label>
      </div>
      <OnboardingActions onBack={onBack} className="mt-4">
        <LargeButton
          disabled={!checked}
          onClick={() =>
            onAccept({
              license: LICENSE_ID,
              licenseUrl: LICENSE_URL,
              termsUrl: TERMS_URL,
              acceptedAt: new Date().toISOString(),
              appVersion: __APP_VERSION__,
            })
          }
        >
          Agree and continue
        </LargeButton>
      </OnboardingActions>
    </>
  );
}
