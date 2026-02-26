Quiero que implementes el frontend web público de GitGov dentro de la carpeta `gitgov-web/` (ya creada), usando Next.js (App Router) + TypeScript + Tailwind.

IMPORTANTE
- Trabaja SOLO dentro de `gitgov-web/`
- NO toques `gitgov/` (Desktop Tauri + React actual)
- NO toques `gitgov/gitgov-server/`
- NO rompas el Golden Path del proyecto (Desktop -> commit/push -> Control Plane)
- Esta web es una app pública separada para:
  1) explicar el producto
  2) servir descarga del Desktop (.exe)
  3) mostrar docs/guías y contacto

Contexto del producto (resumen real)
- GitGov = sistema de gobernanza de Git distribuido
- Componentes actuales:
  - Desktop App (Tauri + React) -> principal para devs
  - Control Plane Server (Rust/Axum) -> centraliza eventos
  - Integraciones -> Jenkins, Jira, GitHub webhooks (en progreso/operativas por fases)
- La propuesta de valor:
  - trazabilidad commit -> CI (Jenkins) -> ticket (Jira)
  - visibilidad de ejecución / compliance / auditoría
- La app web NO reemplaza la app Desktop ni el Control Plane; es sitio público de marketing + descarga + docs

Objetivo del trabajo
Implementar una web pública profesional y usable, con diseño intencional (no plantilla genérica), dentro de `gitgov-web/`.

Stack requerido
- Next.js (App Router)
- TypeScript
- Tailwind CSS
- Sin backend complejo (si hace falta “contacto”, dejar endpoint local mock/placeholder)
- Preparado para deploy en Vercel (pero sin configurar infraestructura real)

Reglas de implementación (importantes)
- Prioriza estructura clara y mantenible
- Usa componentes reutilizables
- Usa diseño consistente (tokens/variables)
- Accesibilidad básica obligatoria:
  - semantic HTML
  - focus visible
  - labels
  - keyboard support
  - reduced motion support donde aplique
- SEO básico obligatorio:
  - metadata por página
  - Open Graph básico
  - title/description buenos
- No metas contenido falso técnico que contradiga el estado real del proyecto
- No inventes endpoints productivos
- Para descargas, usa links/placeholders en `public/downloads/` con comentarios claros

IMPORTANTE DE DISEÑO (NO “AI slop”)
- Evita look genérico tipo SaaS morado estándar
- Define dirección visual clara (tipografía, color, layout)
- Usa CSS variables / tema propio
- Mantén buen look en desktop y mobile
- Animaciones sutiles, no excesivas

Estructura de carpetas ya existente (úsala)
- `gitgov-web/app`
- `gitgov-web/app/(marketing)/features`
- `gitgov-web/app/(marketing)/pricing`
- `gitgov-web/app/(marketing)/download`
- `gitgov-web/app/(marketing)/contact`
- `gitgov-web/app/docs/[[...slug]]`
- `gitgov-web/app/api/download`
- `gitgov-web/app/api/contact`
- `gitgov-web/components/layout`
- `gitgov-web/components/marketing`
- `gitgov-web/components/docs`
- `gitgov-web/components/download`
- `gitgov-web/components/ui`
- `gitgov-web/lib/config`
- `gitgov-web/lib/content`
- `gitgov-web/lib/seo`
- `gitgov-web/lib/analytics`
- `gitgov-web/styles`
- `gitgov-web/public/downloads`
- `gitgov-web/public/images`
- `gitgov-web/public/images/og`
- `gitgov-web/public/icons`
- `gitgov-web/content/docs`
- `gitgov-web/content/blog`
- `gitgov-web/tests/e2e`
- `gitgov-web/tests/unit`

Qué debes crear (mínimo)
1) Proyecto Next.js funcional dentro de `gitgov-web/`
- `package.json`
- `next.config.*`
- `tsconfig.json`
- `postcss.config.*`
- `tailwind.config.*` (si aplica según versión)
- `app/layout.tsx`
- `app/globals.css`
- scripts útiles (`dev`, `build`, `start`, `lint`, `typecheck`)

2) Páginas públicas (MVP)
- Landing/Home (`/`)
- Features (`/features`)
- Download (`/download`)
- Contact (`/contact`)
- Docs index (`/docs`)
- Placeholder Pricing (`/pricing`) aunque sea “Coming soon” elegante
- 404 (`app/not-found.tsx`)

3) Landing con contenido realista de GitGov
Debe comunicar:
- Qué es GitGov (governance + traceability)
- Problema que resuelve (visibilidad/compliance/ejecución)
- Flujo (Desktop -> Control Plane -> Jenkins/Jira/GitHub)
- Valor para roles:
  - CTO/CISO/CETO
  - PM / Engineering Manager
- CTA principal:
  - Descargar Desktop
- CTA secundario:
  - Ver demo / docs

4) Página Download (`/download`)
- Mostrar plataforma principal (Windows)
- Botón de descarga del `.exe`
- Usar una ruta local preparada, por ejemplo:
  - `/downloads/GitGov_0.1.0_x64-setup.exe`
- Si el archivo no existe aún, dejar UI lista + nota “build disponible internamente”
- Incluir checksum/versión como placeholders si no están automatizados
- Mostrar pasos de instalación básicos

5) Página Features (`/features`)
Secciones sugeridas:
- Git operations governance (Desktop flow)
- Control Plane audit logs
- Jenkins CI traceability (Pipeline Health / commit correlation)
- Jira ticket coverage (preview)
- Policy checks (advisory hoy, enforcement futuro)
- Seguridad / append-only / dedupe
- Integraciones y roadmap (sin sobreprometer)

6) Página Contact (`/contact`)
- Form UI (nombre, email, empresa, mensaje)
- `app/api/contact/route.ts`:
  - endpoint local placeholder que valide payload y responda 200
  - sin enviar emails reales
- Mensajes de éxito/error claros
- Dejar preparado para conectar con servicio real después

7) Docs base (`/docs`)
- Render mínimo de docs desde `content/docs` (aunque sea simple)
- `app/docs/[[...slug]]`:
  - fallback para páginas no encontradas
- Crear 2-3 archivos de docs de ejemplo:
  - Introducción a GitGov
  - Instalar Desktop
  - Conectar Control Plane (localhost / 127.0.0.1)
- Puede ser markdown simple o contenido estático, pero deja una base limpia

8) Componentes compartidos
Crear base reusable:
- layout:
  - Header/Nav
  - Footer
  - Container
- marketing:
  - Hero
  - FeatureCard
  - SectionHeader
  - CTASection
  - RoleCards (CTO/PM/etc.)
- download:
  - DownloadCard
  - ReleaseInfo
- ui:
  - Button
  - Badge
  - Card
  - Input
  - Textarea
  - Select (si hace falta)
  - EmptyState

9) Configuración centralizada
Crear utilidades/constantes en `lib/config`:
- nombre del producto
- URLs (GitHub repo, docs, download path)
- versión de Desktop (placeholder configurable)
- copy base reutilizable
Crear `lib/seo` para metadata helpers

10) Analítica (solo scaffold)
- `lib/analytics` con wrapper no-op / placeholder
- No integrar servicios reales aún (GA/Posthog/etc.), solo interfaz lista

11) Calidad mínima
- `npm run build` debe pasar
- `npm run typecheck` debe pasar
- `npm run lint` si se configura
- Añadir README corto en `gitgov-web/` con:
  - cómo correr
  - qué es
  - dónde poner el `.exe` para descarga

12) E2E/Unit tests (scaffold)
- Deja estructura y al menos 1 test placeholder en `tests/unit` y/o `tests/e2e`
- No necesitas cobertura completa ahora

Contenido/Copy: guía de tono (usar)
- Profesional, directo, B2B técnico
- Evitar hype vacío
- Enfatizar:
  - evidencia operativa
  - gobernanza
  - trazabilidad
  - cumplimiento
- No afirmar cosas que aún no estén listas (ej. “GitHub webhook fully managed in production” si no aplica)

Qué NO hacer
- No convertir esto en dashboard autenticado (no duplicar Control Plane)
- No tocar la app Desktop existente
- No meter backend real complejo
- No hardcodear secretos
- No usar diseño genérico de plantilla sin identidad

Orden sugerido de implementación (importante)
1. Inicializar Next.js + Tailwind + config base
2. Crear layout global + navegación + footer + theme
3. Implementar Home + Features + Download + Contact + 404
4. Implementar docs base (`/docs`)
5. Añadir API placeholders (`/api/contact`, `/api/download`)
6. Ajustar responsive + accesibilidad
7. Build/typecheck/lint final
8. README de `gitgov-web`

Entregable esperado (al final)
- Proyecto `gitgov-web/` funcionando con `npm install && npm run dev`
- Sitio público navegable con páginas y diseño coherente
- Botón de descarga preparado para `.exe`
- Base lista para que luego conectemos dominio / deploy

Cuando termines, reporta:
- árbol de archivos clave creados
- comandos para correr
- qué páginas quedaron
- qué placeholders quedaron pendientes (si alguno)
