import { createContext, useContext, useState } from "react";

type Ctx = {
  el: HTMLElement | null;
  setEl: (el: HTMLElement | null) => void;
  centerEl: HTMLElement | null;
  setCenterEl: (el: HTMLElement | null) => void;
  centerActive: boolean;
  setCenterActive: (active: boolean) => void;
};

const HeaderSlotContext = createContext<Ctx>({
  el: null,
  setEl: () => {},
  centerEl: null,
  setCenterEl: () => {},
  centerActive: false,
  setCenterActive: () => {},
});

export function HeaderSlotProvider({ children }: { children: React.ReactNode }) {
  const [el, setEl] = useState<HTMLElement | null>(null);
  const [centerEl, setCenterEl] = useState<HTMLElement | null>(null);
  const [centerActive, setCenterActive] = useState(false);
  return (
    <HeaderSlotContext.Provider
      value={{ el, setEl, centerEl, setCenterEl, centerActive, setCenterActive }}
    >
      {children}
    </HeaderSlotContext.Provider>
  );
}

export function useHeaderSlotRegister() {
  return useContext(HeaderSlotContext).setEl;
}

export function useHeaderSlotElement() {
  return useContext(HeaderSlotContext).el;
}

export function useHeaderCenterSlotRegister() {
  return useContext(HeaderSlotContext).setCenterEl;
}

export function useHeaderCenterSlotElement() {
  return useContext(HeaderSlotContext).centerEl;
}

/** Whether the header should switch to its centered 3-column layout — set by the
 * component (TabsNav) that portals into the center slot. */
export function useHeaderCenterActive() {
  return useContext(HeaderSlotContext).centerActive;
}

export function useHeaderCenterActivate() {
  return useContext(HeaderSlotContext).setCenterActive;
}
