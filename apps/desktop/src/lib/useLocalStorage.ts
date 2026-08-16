import { type Dispatch, type SetStateAction, useEffect, useRef, useState } from "react";

function read<T>(key: string, initial: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? initial : (JSON.parse(raw) as T);
  } catch {
    return initial;
  }
}

/** State backed by localStorage; re-syncs when `key` changes. */
export function useLocalStorage<T>(key: string, initial: T): [T, Dispatch<SetStateAction<T>>] {
  const [value, setValue] = useState<T>(() => read(key, initial));
  const lastKey = useRef(key);
  if (lastKey.current !== key) {
    lastKey.current = key;
    setValue(read(key, initial));
  }
  useEffect(() => {
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch {}
  }, [key, value]);
  return [value, setValue];
}
