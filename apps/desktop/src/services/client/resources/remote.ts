import type { PairingCode, RemoteDevice, RemoteStatus } from "../../../types";
import { call } from "../invoke";

/** Host-global: no method takes a workspace id. Every mutation returns the
 * whole status, so callers seed the cache instead of refetching. */
export class RemoteResource {
  /** False in an open-source build, where the plugin is a stub. */
  available() {
    return call<boolean>("remote_available");
  }

  status() {
    return call<RemoteStatus>("remote_status");
  }

  enable() {
    return call<RemoteStatus>("remote_enable");
  }

  disable() {
    return call<RemoteStatus>("remote_disable");
  }

  setPassword(password: string) {
    return call<RemoteStatus>("remote_set_password", { password });
  }

  beginPairing() {
    return call<PairingCode>("remote_begin_pairing");
  }

  devices() {
    return call<RemoteDevice[]>("remote_devices");
  }

  revoke(id: string) {
    return call<RemoteStatus>("remote_revoke_device", { id });
  }

  /** `read` or `control`. Closes the device's session so it reconnects at the
   * new scope. */
  setScope(id: string, scope: string) {
    return call<RemoteStatus>("remote_set_device_scope", { id, scope });
  }
}
