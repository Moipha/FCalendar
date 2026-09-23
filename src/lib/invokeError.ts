export function invokeErrorMessage(error: unknown): string {
  if (typeof error === "string" && error.trim()) {
    return error;
  }
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message: unknown }).message;
    if (typeof message === "string" && message.trim()) {
      return message;
    }
  }
  return String(error);
}

/** 前端控制台打出完整 Rust/invoke 错误，并返回给界面。 */
export function reportInvokeError(context: string, error: unknown): string {
  const message = invokeErrorMessage(error);
  console.error(`[${context}]`, message, error);
  return message;
}
