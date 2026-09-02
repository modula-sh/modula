import { useMutation, useQueryClient } from "@tanstack/react-query";
import { QRCodeSVG } from "qrcode.react";
import { useEffect, useRef, useState } from "react";
import { remoteKeys, useRemoteDevices } from "../../queries/remote";
import { client, errorMessage } from "../../services/client";
import { BaseModal } from "../BaseModal";
import { Button } from "../Button";

function mmss(ms: number) {
  const total = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}

/** Mounts fresh per open; minting again just replaces the pending token. */
export function PairDeviceModal({ onClose }: { onClose: () => void }) {
  const queryClient = useQueryClient();
  const { data: devices } = useRemoteDevices(2000);
  const [now, setNow] = useState(() => Date.now());
  const mint = useMutation({ mutationFn: () => client.remote.beginPairing() });
  const { mutate } = mint;

  useEffect(() => mutate(), [mutate]);

  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);

  const known = useRef<Set<string> | null>(null);
  if (!known.current && devices) known.current = new Set(devices.map((d) => d.id));
  const paired = devices?.find((d) => known.current?.has(d.id) === false);

  const remaining = mint.data ? mint.data.expires_at * 1000 - now : 0;
  const close = () => {
    queryClient.invalidateQueries({ queryKey: remoteKeys.devices() });
    onClose();
  };

  return (
    <BaseModal open onCancel={close} panelClassName="w-[22rem]">
      <div className="text-sm font-semibold text-fg">Pair a device</div>
      <div className="text-xs text-fg-muted">
        Scan this with the Modula app, then enter your remote password on the phone.
      </div>

      <div className="flex flex-col items-center gap-2 py-1">
        {mint.isPending && <div className="text-xs text-fg-muted">Minting a code…</div>}
        {mint.error && <div className="text-xs text-red-400">{errorMessage(mint.error)}</div>}
        {mint.data &&
          (remaining > 0 ? (
            <>
              {/* Dark-on-light in both themes — theme tokens would invert the code. */}
              <div className="rounded-lg bg-white p-3">
                <QRCodeSVG
                  value={mint.data.qr_payload}
                  size={200}
                  level="M"
                  bgColor="#ffffff"
                  fgColor="#000000"
                />
              </div>
              <div className="text-xs text-fg-muted">Expires in {mmss(remaining)}</div>
            </>
          ) : (
            <div className="text-xs text-fg-muted">This code expired. Regenerate to try again.</div>
          ))}
        {paired && <div className="text-[11px] text-green-400">{paired.name} paired.</div>}
      </div>

      <div className="flex items-center gap-2">
        <Button onClick={() => mutate()} disabled={mint.isPending}>
          Regenerate
        </Button>
        <Button tone="link" onClick={close}>
          Done
        </Button>
      </div>
    </BaseModal>
  );
}
