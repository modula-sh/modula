import { createContext, useContext } from "react";
import { useKeptElement } from "../lib/useKeptElement";

const Ctx = createContext<HTMLDivElement | null>(null);

export function ModalPortalProvider({
  className,
  children,
}: {
  className?: string;
  children: React.ReactNode;
}) {
  const [node, setNode] = useKeptElement<HTMLDivElement>();
  return (
    <Ctx.Provider value={node}>
      <div ref={setNode} className={className}>
        {children}
      </div>
    </Ctx.Provider>
  );
}

export function useModalPortal() {
  return useContext(Ctx);
}
