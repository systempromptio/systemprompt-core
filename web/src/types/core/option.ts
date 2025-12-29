type Option<T> =
  | { readonly some: true; readonly value: T }
  | { readonly some: false }

function Some<T>(value: T): Option<T> {
  return { some: true, value }
}

const None: Option<never> = { some: false }

function isSome<T>(option: Option<T>): option is { readonly some: true; readonly value: T } {
  return option.some
}

function isNone<T>(option: Option<T>): option is { readonly some: false } {
  return !option.some
}

function unwrapOption<T>(option: Option<T>): T {
  if (option.some) {
    return option.value
  }
  throw new Error('Called unwrap on a None value')
}

function unwrapOptionOr<T>(option: Option<T>, defaultValue: T): T {
  return option.some ? option.value : defaultValue
}

function mapOption<T, U>(option: Option<T>, fn: (value: T) => U): Option<U> {
  return option.some ? Some(fn(option.value)) : None
}

function flatMapOption<T, U>(option: Option<T>, fn: (value: T) => Option<U>): Option<U> {
  return option.some ? fn(option.value) : None
}

function fromNullable<T>(value: T | undefined): Option<T> {
  return value !== undefined ? Some(value) : None
}

function toNullable<T>(option: Option<T>): T | undefined {
  return option.some ? option.value : undefined
}

function filter<T>(option: Option<T>, predicate: (value: T) => boolean): Option<T> {
  return option.some && predicate(option.value) ? option : None
}

export type { Option }
export {
  Some,
  None,
  isSome,
  isNone,
  unwrapOption,
  unwrapOptionOr,
  mapOption,
  flatMapOption,
  fromNullable,
  toNullable,
  filter,
}
