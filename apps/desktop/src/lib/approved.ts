import type { Approved } from "../types";

export function approvedTone(a: Approved): "green" | "red" | "zinc" {
  if (a === true) return "green";
  if (a === false) return "red";
  return "zinc";
}

export function approvedLabel(a: Approved): "approved" | "rejected" | "pending" {
  if (a === true) return "approved";
  if (a === false) return "rejected";
  return "pending";
}
