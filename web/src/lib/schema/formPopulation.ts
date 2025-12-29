import type { FormValues } from './types'

export function populateFormFromObject(obj: unknown): FormValues {
    if (typeof obj !== 'object' || obj === null) return {}

    return obj as FormValues
}
