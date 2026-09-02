import { useQuery } from "@tanstack/react-query";
import { client } from "../services/client";

export const remoteKeys = {
  available: () => ["remote", "available"] as const,
  status: () => ["remote", "status"] as const,
  devices: () => ["remote", "devices"] as const,
};

/** Compiled-in, so it never changes for the life of the process. */
export function useRemoteAvailable() {
  return useQuery({
    queryKey: remoteKeys.available(),
    queryFn: () => client.remote.available(),
    staleTime: Infinity,
  });
}

export function useRemoteStatus() {
  return useQuery({
    queryKey: remoteKeys.status(),
    queryFn: () => client.remote.status(),
  });
}

/** `connected` is live endpoint state, so the pairing modal polls while open. */
export function useRemoteDevices(pollMs?: number) {
  return useQuery({
    queryKey: remoteKeys.devices(),
    queryFn: () => client.remote.devices(),
    refetchInterval: pollMs,
  });
}
