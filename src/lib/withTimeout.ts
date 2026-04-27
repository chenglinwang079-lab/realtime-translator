export class TimeoutError extends Error {
  constructor(operation: string, ms: number) {
    super(`${operation}超时（${ms / 1000}秒）`);
    this.name = "TimeoutError";
  }
}

export function withTimeout<T>(
  promise: Promise<T>,
  ms: number,
  operation: string,
): Promise<T> {
  // 初始化为 0（Node 类型），clearTimeout(0) 是 no-op，安全
  let timer: ReturnType<typeof setTimeout> = 0 as unknown as ReturnType<typeof setTimeout>;
  return Promise.race([
    promise.finally(() => clearTimeout(timer)),
    new Promise<never>((_, reject) => {
      timer = setTimeout(() => reject(new TimeoutError(operation, ms)), ms);
    }),
  ]);
}
