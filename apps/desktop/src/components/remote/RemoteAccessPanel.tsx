import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useFeedback } from "../../hooks/useFeedback";
import { remoteKeys, useRemoteDevices, useRemoteStatus } from "../../queries/remote";
import { client, errorMessage } from "../../services/client";
import type { RemoteDevice, RemoteStatus } from "../../types";
import { Button } from "../Button";
import { ConfirmModal } from "../ConfirmModal";
import { FeedbackText } from "../FeedbackText";
import { FieldRow } from "../FieldRow";
import { openUrl } from "../openUrl";
import { Pill } from "../Pill";
import { PromptModal } from "../PromptModal";
import { SegmentedControl } from "../SegmentedControl";
import { Switch } from "../Switch";
import { TimeAgo } from "../TimeAgo";
import { PairDeviceModal } from "./PairDeviceModal";

const DOCS_URL = "https://github.com/modula-sh/modula/blob/main/docs/REMOTE.md";
/** `MIN_PASSWORD_LEN` in the engine owns the rule; this only quotes it. */
const MIN_PASSWORD_LEN = 8;

/** Host-global — remote access is a property of this machine, not a workspace. */
export function RemoteAccessPanel() {
  const queryClient = useQueryClient();
  const fb = useFeedback();
  const { data: status, error } = useRemoteStatus();
  const { data: devices } = useRemoteDevices();
  const [passwordOpen, setPasswordOpen] = useState(false);
  const [password, setPassword] = useState("");
  const [passwordError, setPasswordError] = useState<string | null>(null);
  const [revoking, setRevoking] = useState<RemoteDevice | null>(null);
  const [granting, setGranting] = useState<RemoteDevice | null>(null);
  const [pairing, setPairing] = useState(false);

  // Every mutating RPC returns the whole status, so nothing needs a refetch.
  const applyStatus = (next: RemoteStatus) => queryClient.setQueryData(remoteKeys.status(), next);
  const onError = (e: unknown) => fb.err(errorMessage(e), { clearAfter: 8000 });

  const toggle = useMutation({
    mutationFn: (on: boolean) => (on ? client.remote.enable() : client.remote.disable()),
    onSuccess: applyStatus,
    onError,
  });
  const savePassword = useMutation({
    mutationFn: (value: string) => client.remote.setPassword(value),
    onSuccess: (next) => {
      applyStatus(next);
      setPasswordOpen(false);
      setPassword("");
    },
    onError: (e) => setPasswordError(errorMessage(e)),
  });
  const setScope = useMutation({
    mutationFn: ({ id, scope }: { id: string; scope: string }) => client.remote.setScope(id, scope),
    onSuccess: (next) => {
      applyStatus(next);
      queryClient.invalidateQueries({ queryKey: remoteKeys.devices() });
      setGranting(null);
    },
    onError: (e) => {
      setGranting(null);
      onError(e);
    },
  });
  const revoke = useMutation({
    mutationFn: (id: string) => client.remote.revoke(id),
    onSuccess: (next) => {
      applyStatus(next);
      queryClient.invalidateQueries({ queryKey: remoteKeys.devices() });
      setRevoking(null);
    },
    onError: (e) => {
      setRevoking(null);
      onError(e);
    },
  });

  if (error) return <p className="text-sm text-red-400">{errorMessage(error)}</p>;
  if (!status) return null;

  return (
    <div className="space-y-2">
      <section className="border border-card-border/50 bg-card rounded-xl px-3">
        <FieldRow
          label="Enable remote access"
          description={
            status.password_set
              ? "Lets a paired phone reach this host over an encrypted peer-to-peer link."
              : `Set a password first — at least ${MIN_PASSWORD_LEN} characters.`
          }
        >
          <div className="flex items-center gap-2">
            {status.running && <Pill tone="green">running</Pill>}
            <Switch
              label="Enable remote access"
              checked={status.enabled}
              disabled={!status.password_set || toggle.isPending}
              onChange={(on) => {
                fb.clear();
                toggle.mutate(on);
              }}
            />
          </div>
        </FieldRow>

        <FieldRow
          label="Password"
          description={`Devices enter this once when pairing. At least ${MIN_PASSWORD_LEN} characters; never shown again.`}
        >
          <Button
            onClick={() => {
              setPassword("");
              setPasswordError(null);
              setPasswordOpen(true);
            }}
          >
            {status.password_set ? "Change password" : "Set password"}
          </Button>
        </FieldRow>

        <FieldRow
          label="Pair a device"
          description={
            status.running
              ? "Show a QR code for the Modula app to scan."
              : "Turn remote access on to pair."
          }
        >
          <Button disabled={!status.running} onClick={() => setPairing(true)}>
            Pair a device
          </Button>
        </FieldRow>

        <div className="py-3 border-b border-border/60 last:border-b-0 space-y-2">
          <div className="text-fg font-inter text-xs">Paired devices</div>
          {devices?.length ? (
            devices.map((d) => (
              <div key={d.id} className="flex items-center gap-2 text-xs">
                <span className="text-fg truncate">{d.name}</span>
                <Pill size="sm">{d.platform}</Pill>
                {d.connected && (
                  <Pill size="sm" tone="green">
                    connected
                  </Pill>
                )}
                <TimeAgo iso={d.last_seen_at} className="ml-auto text-[11px] text-fg-subtle" />
                <SegmentedControl>
                  <Button
                    tone="tab"
                    active={d.scope !== "control"}
                    onClick={() => setScope.mutate({ id: d.id, scope: "read" })}
                  >
                    Read
                  </Button>
                  <Button
                    tone="tab"
                    active={d.scope === "control"}
                    onClick={() => d.scope !== "control" && setGranting(d)}
                  >
                    Control
                  </Button>
                </SegmentedControl>
                <Button tone="link" onClick={() => setRevoking(d)}>
                  Revoke
                </Button>
              </div>
            ))
          ) : (
            <div className="text-fg-muted text-[11px]">No devices paired yet.</div>
          )}
        </div>
      </section>

      {status.enabled && !status.running && status.last_error && (
        <p className="text-[11px] text-red-400">{status.last_error}</p>
      )}
      <FeedbackText feedback={fb.feedback} />
      <p className="text-[11px] text-fg-subtle">
        This host must stay awake with Modula running for a phone to reach it.{" "}
        <button type="button" onClick={() => openUrl(DOCS_URL)} className="underline hover:text-fg">
          Learn more
        </button>
      </p>

      <PromptModal
        open={passwordOpen}
        title={status.password_set ? "Change remote password" : "Set remote password"}
        type="password"
        value={password}
        onChange={setPassword}
        placeholder={`at least ${MIN_PASSWORD_LEN} characters`}
        confirmLabel="Save"
        busy={savePassword.isPending}
        error={passwordError}
        onConfirm={() => {
          setPasswordError(null);
          savePassword.mutate(password);
        }}
        onCancel={() => setPasswordOpen(false)}
      />
      {pairing && <PairDeviceModal onClose={() => setPairing(false)} />}
      <ConfirmModal
        open={!!granting}
        title={`Give ${granting?.name ?? ""} control?`}
        body="Control lets this device do anything the app offers beyond reading — now and as the app grows. It still cannot reach remote access settings or anything holding a credential. Its session is closed so it reconnects at the new scope."
        confirmLabel="Give control"
        busy={setScope.isPending}
        onConfirm={() => granting && setScope.mutate({ id: granting.id, scope: "control" })}
        onCancel={() => setGranting(null)}
      />
      <ConfirmModal
        open={!!revoking}
        title={`Revoke ${revoking?.name ?? ""}?`}
        body="Any live session is closed immediately and the device must pair again to reconnect."
        confirmLabel="Revoke"
        busy={revoke.isPending}
        onConfirm={() => revoking && revoke.mutate(revoking.id)}
        onCancel={() => setRevoking(null)}
      />
    </div>
  );
}
