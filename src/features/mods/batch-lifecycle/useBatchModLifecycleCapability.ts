import { useEffect, useState } from "react";
import { getBatchModLifecycleCapability } from "./batchModLifecycleApi.ts";
import type { BatchModLifecycleCapabilityDto } from "./batchModLifecycleTypes.ts";

export type BatchModLifecycleCapabilityState = {
  status: "loading" | "ready";
  capability: BatchModLifecycleCapabilityDto | null;
};

export const UNAVAILABLE_BATCH_CAPABILITY: BatchModLifecycleCapabilityDto = {
  previewAvailable: false,
  writeAvailable: false,
  unavailableReasonCode: "batch_capability_unavailable",
};

export function useBatchModLifecycleCapability(): BatchModLifecycleCapabilityState {
  const [state, setState] = useState<BatchModLifecycleCapabilityState>({
    status: "loading",
    capability: null,
  });

  useEffect(() => {
    let disposed = false;
    void getBatchModLifecycleCapability()
      .then((capability) => {
        if (!disposed) setState({ status: "ready", capability });
      })
      .catch(() => {
        if (!disposed) {
          setState({
            status: "ready",
            capability: UNAVAILABLE_BATCH_CAPABILITY,
          });
        }
      });
    return () => {
      disposed = true;
    };
  }, []);

  return state;
}
