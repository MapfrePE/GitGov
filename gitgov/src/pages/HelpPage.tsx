import { useState } from 'react'
import { Header } from '@/components/layout/Header'
import { ChevronDown, ChevronRight, Shield, Database, Eye, Lock, Server, HelpCircle, ExternalLink } from 'lucide-react'
import clsx from 'clsx'

interface FaqItem {
  q: string
  a: string
}

interface FaqSection {
  title: string
  icon: React.ElementType
  items: FaqItem[]
}

const faqData: FaqSection[] = [
  {
    title: 'Qué GitGov NO hace',
    icon: Shield,
    items: [
      {
        q: '¿GitGov lee mi código fuente?',
        a: 'No. GitGov solo captura metadatos: tipo de evento, SHA del commit, rama, autor, timestamp, conteo de archivos y nombre del repo. El código fuente, contenido de archivos, diffs y cuerpos de mensajes de commit nunca se transmiten y nunca abandonan tu estación de trabajo.',
      },
      {
        q: '¿GitGov monitoriza mi pantalla, teclado o aplicaciones?',
        a: 'No. GitGov solo observa operaciones Git (commit, push, creación de ramas). No tiene acceso a tu pantalla, portapapeles, navegador, IDE ni ninguna aplicación fuera de Git.',
      },
      {
        q: '¿GitGov analiza la calidad del código?',
        a: 'No. GitGov no hace lint, review ni evalúa la calidad de tu código. Captura metadatos sobre cuándo y dónde ocurren los eventos Git, no qué contiene el código.',
      },
      {
        q: '¿GitGov reemplaza CI/CD?',
        a: 'No. GitGov se integra con herramientas CI/CD (Jenkins, GitHub Actions) para correlacionar commits con pipelines. No ejecuta builds, tests ni despliegues.',
      },
      {
        q: '¿GitGov bloquea operaciones Git?',
        a: 'No. GitGov es una herramienta de detección y observabilidad, no de enforcement. Puede señalar que ocurrió un push a una rama protegida, pero no impide que el push se realice.',
      },
      {
        q: '¿GitGov toma decisiones de RRHH?',
        a: 'No. Las señales son observaciones consultivas — indican que una regla se activó. No establecen intención, negligencia ni culpa. La organización es plenamente responsable de cualquier decisión basada en señales.',
      },
      {
        q: '¿GitGov perfila productividad individual?',
        a: 'No. No hay "líneas de código por día", "puntuaciones de commits" ni rankings de productividad. GitGov es una herramienta de gobernanza y cumplimiento, no de rendimiento.',
      },
    ],
  },
  {
    title: 'Datos y seguridad',
    icon: Lock,
    items: [
      {
        q: '¿Dónde se almacenan mis datos?',
        a: 'Los eventos se almacenan en una base de datos PostgreSQL controlada por tu organización (Supabase o self-hosted). La app de escritorio mantiene un outbox local SQLite para resiliencia offline.',
      },
      {
        q: '¿Mis datos están cifrados?',
        a: 'Sí, en múltiples capas: TLS (HTTPS) en tránsito entre Desktop y Control Plane; AES-256 en reposo en las bases de datos Supabase; API keys almacenadas como hashes SHA-256; y en tu estación de trabajo, las claves se guardan en el keyring del SO (Windows DPAPI, macOS Keychain, Linux Secret Service).',
      },
      {
        q: '¿Se pueden modificar o eliminar registros de auditoría?',
        a: 'No. Los registros son append-only por diseño. La API no expone UPDATE ni DELETE sobre tablas de eventos. Cada exportación se registra también como evento de auditoría.',
      },
      {
        q: '¿Quién puede ver mis eventos?',
        a: 'El acceso se controla con RBAC: los Developers solo ven sus propios eventos. Los Admins ven todos los eventos, estadísticas y dashboard. No hay forma de que un Developer acceda a eventos de otro desarrollador.',
      },
      {
        q: '¿Cómo se protegen las API keys?',
        a: 'Se hashean con SHA-256 antes de almacenarse en la base de datos. La clave en texto plano solo se muestra una vez al crearla. En el escritorio, se almacenan en el keyring del sistema operativo.',
      },
      {
        q: '¿GitGov vende o comparte mis datos?',
        a: 'No. Todos los datos pertenecen a tu organización. GitGov no tiene modelo de monetización de datos. Los datos no se comparten con terceros.',
      },
    ],
  },
  {
    title: 'App de escritorio',
    icon: Database,
    items: [
      {
        q: '¿Qué plataformas soporta GitGov Desktop?',
        a: 'GitGov Desktop está construido con Tauri y soporta Windows, macOS y Linux.',
      },
      {
        q: '¿Qué pasa si pierdo la conexión a internet?',
        a: 'Los eventos se encolan en un outbox local SQLite y se sincronizan automáticamente cuando se restablece la conectividad. No se pierde ningún evento.',
      },
      {
        q: '¿Cómo configuro las políticas de gobernanza?',
        a: 'Las políticas se definen en un archivo gitgov.toml en la raíz de tu repositorio. Puedes verlo desde la pestaña Configuración de esta app.',
      },
    ],
  },
  {
    title: 'Control Plane',
    icon: Server,
    items: [
      {
        q: '¿Qué es el Control Plane?',
        a: 'Es el servidor central Axum (Rust) que recibe eventos de los clientes desktop, procesa webhooks de GitHub/Jenkins/Jira, ejecuta verificaciones de política y sirve el dashboard de administración.',
      },
      {
        q: '¿Puedo self-hostear el Control Plane?',
        a: 'Sí. Puede desplegarse en cualquier servidor que ejecute binarios Rust. Requiere una base de datos PostgreSQL.',
      },
      {
        q: '¿Qué integraciones están soportadas?',
        a: 'GitHub (webhooks de push y ramas, audit log streaming), Jenkins (ingesta de pipelines y correlación commit-pipeline), y Jira (ingesta de tickets, correlación commit-ticket y reportes de cobertura).',
      },
    ],
  },
  {
    title: 'Cumplimiento',
    icon: Eye,
    items: [
      {
        q: '¿GitGov ayuda con SOC 2?',
        a: 'Sí. GitGov proporciona pistas de auditoría append-only, control de acceso basado en roles y registros de eventos inmutables — controles clave para SOC 2 Tipo II.',
      },
      {
        q: '¿GitGov ayuda con RGPD?',
        a: 'Sí. Se diseña con principios RGPD: minimización de datos (solo metadatos), derecho de acceso (desarrolladores ven sus propios eventos), portabilidad (POST /export) y distinción responsable/encargado.',
      },
    ],
  },
]

function FaqAccordion({ section }: { section: FaqSection }) {
  const [openIndex, setOpenIndex] = useState<number | null>(null)
  const Icon = section.icon

  return (
    <section className="rounded-2xl border border-surface-700/30 bg-surface-800/40 overflow-hidden">
      <div className="px-5 py-4 border-b border-surface-700/20 flex items-center gap-2.5">
        <div className="w-7 h-7 rounded-lg bg-brand-600/15 flex items-center justify-center">
          <Icon size={14} strokeWidth={1.5} className="text-brand-400" />
        </div>
        <h2 className="text-[13px] font-semibold text-white">{section.title}</h2>
      </div>
      <div className="divide-y divide-surface-700/20">
        {section.items.map((item, i) => {
          const isOpen = openIndex === i
          return (
            <div key={i}>
              <button
                onClick={() => setOpenIndex(isOpen ? null : i)}
                className="w-full flex items-center gap-3 px-5 py-3.5 text-left hover:bg-white/[0.02] transition-colors"
              >
                {isOpen ? (
                  <ChevronDown size={14} className="text-brand-400 flex-shrink-0" />
                ) : (
                  <ChevronRight size={14} className="text-surface-500 flex-shrink-0" />
                )}
                <span className={clsx(
                  'text-[12px] font-medium transition-colors',
                  isOpen ? 'text-white' : 'text-surface-300'
                )}>
                  {item.q}
                </span>
              </button>
              {isOpen && (
                <div className="px-5 pb-4 pl-12">
                  <p className="text-[11px] leading-relaxed text-surface-400">
                    {item.a}
                  </p>
                </div>
              )}
            </div>
          )
        })}
      </div>
    </section>
  )
}

export function HelpPage() {
  return (
    <div className="h-full flex flex-col bg-surface-950">
      <Header />

      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-2xl mx-auto space-y-5 animate-fade-in">
          {/* Header */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2.5">
              <HelpCircle size={16} className="text-brand-400" />
              <h1 className="text-[15px] font-semibold text-white">Ayuda y FAQ</h1>
            </div>
            <a
              href="https://git-gov.vercel.app/docs/faq"
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-1.5 text-[10px] text-brand-400 hover:text-brand-300 transition-colors"
            >
              Ver documentación completa
              <ExternalLink size={10} />
            </a>
          </div>

          {/* Info banner */}
          <div className="rounded-xl border border-brand-500/20 bg-brand-500/5 px-4 py-3">
            <p className="text-[11px] text-brand-300 font-medium mb-1">
              Principio fundamental de GitGov
            </p>
            <p className="text-[10px] text-surface-400 leading-relaxed">
              Solo metadatos, nunca código fuente. El contenido de tus archivos, diffs, mensajes de commit, contraseñas y secretos nunca se transmiten ni abandonan tu estación de trabajo. Esta es una garantía arquitectónica, no una opción de configuración.
            </p>
          </div>

          {/* FAQ Sections */}
          {faqData.map((section) => (
            <FaqAccordion key={section.title} section={section} />
          ))}

          {/* Footer */}
          <div className="text-center py-4">
            <p className="text-[10px] text-surface-600">
              ¿Más preguntas? Contacta a tu administrador o visita{' '}
              <a
                href="https://git-gov.vercel.app/contact"
                target="_blank"
                rel="noopener noreferrer"
                className="text-brand-400 hover:text-brand-300 transition-colors"
              >
                git-gov.vercel.app/contact
              </a>
            </p>
          </div>
        </div>
      </div>
    </div>
  )
}
