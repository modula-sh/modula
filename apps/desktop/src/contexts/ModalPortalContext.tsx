import { createContext, useContext, useState } from "react";

const Ctx = createContext<HTMLDivElement | null>(null);

export function ModalPortalProvider({
  className,
  children,
}: {
  className?: string;
  children: React.ReactNode;
}) {
  const [node, setNode] = useState<HTMLDivElement | null>(null);
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
