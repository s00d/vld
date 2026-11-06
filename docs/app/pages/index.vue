<script setup lang="ts">
import type { ButtonProps, PageFeatureProps } from '@nuxt/ui'

const { data: page } = await useAsyncData('landing', () => queryCollection('landing').path('/').first())

useSeoMeta({
  title: 'vld',
  description: page.value?.description,
  ogTitle: 'vld',
  ogDescription: page.value?.description,
})

const heroLinks: ButtonProps[] = [
  { label: 'Get started', to: '/getting-started/introduction', size: 'lg', trailingIcon: 'i-lucide-arrow-right' },
  { label: 'GitHub', to: 'https://github.com/s00d/vld', target: '_blank', size: 'lg', color: 'neutral', variant: 'outline', icon: 'i-simple-icons-github' },
]

const features: PageFeatureProps[] = [
  { title: 'Schemas as values', description: 'Chain checks on strings, numbers, dates, collections and objects, then parse JSON or serde values into typed Rust.', icon: 'i-lucide-shapes', to: '/schemas/strings' },
  { title: 'Typed structs', description: 'schema! and #[derive(Validate)] generate parse(), field-level errors and lenient parsing for your own types.', icon: 'i-lucide-braces', to: '/structs/schema-macro' },
  { title: 'Every error at once', description: 'Typed issue codes with paths, flatten/treeify/prettify helpers, custom messages and bundled translations.', icon: 'i-lucide-list-checks', to: '/errors/overview' },
  { title: 'OpenAPI from the same code', description: 'JSON Schema and OpenAPI 3.1 documents, nested components and breaking-change diffs.', icon: 'i-lucide-file-json', to: '/openapi/json-schema' },
  { title: 'Web extractors', description: 'Axum, Actix, ntex, Poem, Rama, Rocket, Salvo, Tower, Warp and Tonic with one error format and body limits.', icon: 'i-lucide-globe', to: '/integrations/web/overview' },
  { title: 'Beyond HTTP', description: 'Databases, message queues, config files, CLI arguments, UI frameworks, TypeScript codegen and fake data.', icon: 'i-lucide-plug', to: '/integrations/database/sqlx' },
]

const quickLinks: ButtonProps[] = [
  { label: 'Quick start', to: '/getting-started/quick-start', trailingIcon: 'i-lucide-arrow-right' },
  { label: 'Migrate from 0.4', to: '/guides/migration', color: 'neutral', variant: 'outline' },
]
</script>

<template>
  <div>
    <UPageHero
      title="Runtime validation for Rust, the Zod way"
      description="vld describes the shape of your data once and checks untrusted input against it: JSON bodies, query strings, config files, database rows. Errors list every problem with its path, and the same schema produces OpenAPI."
      :links="heroLinks"
    >
      <template #headline>
        <UButton to="/guides/migration" label="vld 0.5 is out: what changed" trailing-icon="i-lucide-arrow-right" color="neutral" variant="subtle" size="sm" />
      </template>
    </UPageHero>

    <UPageSection title="What you get" :features="features" />

    <UPageSection
      title="Two minutes to the first parse"
      description="Add the crate, describe a struct and call parse(). Invalid input returns a VldError with every issue, not just the first."
      orientation="horizontal"
      :links="quickLinks"
    >
      <div class="min-w-0">
        <ContentRenderer v-if="page" :value="page" />
      </div>
    </UPageSection>
  </div>
</template>
