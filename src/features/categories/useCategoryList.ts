import { useCallback, useEffect, useState } from "react";
import { listCategories, type CategoryItem } from "./categoryApi";

export type CategoryListState =
  | { status: "loading" }
  | { status: "ready"; categories: CategoryItem[] }
  | { status: "error" };

export function useCategoryList() {
  const [state, setState] = useState<CategoryListState>({ status: "loading" });
  const [refreshToken, setRefreshToken] = useState(0);

  const refresh = useCallback(() => {
    setRefreshToken((current) => current + 1);
  }, []);

  useEffect(() => {
    let cancelled = false;
    setState({ status: "loading" });

    void listCategories()
      .then((categories) => {
        if (!cancelled) {
          setState({ status: "ready", categories });
        }
      })
      .catch(() => {
        if (!cancelled) {
          setState({ status: "error" });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [refreshToken]);

  return { state, refresh };
}
