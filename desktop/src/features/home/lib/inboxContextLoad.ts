export const INBOX_CONTEXT_RETRY_DELAYS_MS = [250, 750] as const;

export async function retryInboxContextRequest<T>(
  request: () => Promise<T>,
  wait: (delayMs: number) => Promise<void> = (delayMs) =>
    new Promise((resolve) => window.setTimeout(resolve, delayMs)),
): Promise<T> {
  let lastError: unknown;
  for (
    let attempt = 0;
    attempt <= INBOX_CONTEXT_RETRY_DELAYS_MS.length;
    attempt += 1
  ) {
    try {
      return await request();
    } catch (error) {
      lastError = error;
      const delayMs = INBOX_CONTEXT_RETRY_DELAYS_MS[attempt];
      if (delayMs === undefined) break;
      await wait(delayMs);
    }
  }
  throw lastError;
}

export function shouldReportInboxContextLoadError({
  ancestorFailed,
  descendantFailed,
  loadedContextCount,
}: {
  ancestorFailed: boolean;
  descendantFailed: boolean;
  loadedContextCount: number;
}) {
  return ancestorFailed && descendantFailed && loadedContextCount === 0;
}
