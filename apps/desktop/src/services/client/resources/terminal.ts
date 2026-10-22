import type { Channel } from "@tauri-apps/api/core";
import { call } from "../invoke";

/** The desktop-hosted PTY behind a terminal-mode conversation. */
export class TerminalResource {
  /** Attach to the conversation's PTY, replaying its output, or launch it at `cols`×`rows`. */
  open(
    ws: string,
    conversationId: string,
    prompt: string | undefined,
    cols: number,
    rows: number,
    onData: Channel<ArrayBuffer>,
    onExit: Channel<number>,
  ) {
    return call<void>("pty_open", {
      workspaceId: ws,
      conversationId,
      prompt,
      cols,
      rows,
      onData,
      onExit,
    });
  }

  write(conversationId: string, data: string) {
    return call<void>("pty_write", { conversationId, data });
  }

  resize(conversationId: string, cols: number, rows: number) {
    return call<void>("pty_resize", { conversationId, cols, rows });
  }
}
