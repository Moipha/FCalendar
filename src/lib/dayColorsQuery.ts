/** 与 Vue Query 一致的日格颜色缓存键（避免 computed 对象引用不一致）。 */
export function dayColorsQueryKey(from: string, to: string) {
  return ["dayColors", from, to] as const;
}
