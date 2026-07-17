export type DeferredOperation<T> = () => T | PromiseLike<T>;

export function runDeferred<T>(operation: DeferredOperation<T>): Promise<T> {
  return Promise.resolve().then(operation);
}

type LatestRequestCallbacks<T> = {
  onSuccess: (value: T) => void;
  onFailure: () => void;
};

export function createLatestRequestController() {
  let generation = 0;

  return {
    run<T>(operation: DeferredOperation<T>, callbacks: LatestRequestCallbacks<T>): Promise<void> {
      const requestGeneration = ++generation;
      return runDeferred(operation).then(
        (value) => {
          if (requestGeneration === generation) callbacks.onSuccess(value);
        },
        () => {
          if (requestGeneration === generation) callbacks.onFailure();
        },
      );
    },
    invalidate() {
      generation += 1;
    },
  };
}

export function createSingleFlightController() {
  let active = false;

  return {
    run<T>(operation: DeferredOperation<T>): Promise<T> | null {
      if (active) return null;
      active = true;
      return runDeferred(operation).finally(() => {
        active = false;
      });
    },
  };
}
