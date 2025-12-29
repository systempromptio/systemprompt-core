import { useMemo, useCallback } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import type { JSONSchema7 } from 'json-schema'
import type { FormValues, FieldValue } from '@/lib/schema/types'
import { isRequired } from '@/lib/schema/types'
import { getDiscriminatorField } from '@/lib/schema/resolver'
import { populateFormFromObject } from '@/lib/schema/formPopulation'
import { SchemaField } from './SchemaField'

interface SchemaFormGeneratorProps {
  schema: JSONSchema7
  values: FormValues
  onChange: Dispatch<SetStateAction<FormValues>>
  errors?: Record<string, string>
  onLoadingChange?: (loading: boolean) => void
}

export function SchemaFormGenerator({
  schema,
  values,
  onChange,
  errors = {},
  onLoadingChange,
}: SchemaFormGeneratorProps) {
  const discriminatorField = useMemo(() => {
    return getDiscriminatorField(schema)
  }, [schema])

  const handleFieldChange = (fieldName: string) => (value: FieldValue) => {
    onChange({
      ...values,
      [fieldName]: value,
    })
  }

  const handleObjectSelect = useCallback(async (obj: unknown) => {
    const populated = populateFormFromObject(obj)

    onChange((currentValues) => ({
      ...currentValues,
      ...populated
    }))
  }, [onChange])

  const sortedFields = Object.entries(schema.properties || {}).sort(([nameA], [nameB]) => {
    if (discriminatorField) {
      if (nameA === discriminatorField) return -1
      if (nameB === discriminatorField) return 1
    }

    const requiredA = isRequired(nameA, schema)
    const requiredB = isRequired(nameB, schema)

    if (requiredA && !requiredB) return -1
    if (!requiredA && requiredB) return 1

    return nameA.localeCompare(nameB)
  })

  return (
    <div className="space-y-2">
      {sortedFields.map(([fieldName, property]) => {
        if (typeof property === 'boolean') return null

        return (
          <SchemaField
            key={fieldName}
            name={fieldName}
            property={property}
            value={values[fieldName]}
            onChange={handleFieldChange(fieldName)}
            onObjectSelect={handleObjectSelect}
            onLoadingChange={onLoadingChange}
            error={errors[fieldName]}
            errors={errors}
            required={isRequired(fieldName, schema)}
          />
        )
      })}

      {Object.keys(errors).length > 0 && Object.keys(schema.properties || {}).length === 0 && (
        <div className="p-3 bg-red-50 border border-red-200 rounded-md">
          <p className="text-sm text-red-600">Please correct the errors above</p>
        </div>
      )}
    </div>
  )
}
