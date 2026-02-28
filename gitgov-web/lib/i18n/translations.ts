export type Locale = 'en' | 'es';

export const translations = {
    // ═══ Navigation ═══
    'nav.features': { en: 'Features', es: 'Características' },
    'nav.download': { en: 'Download', es: 'Descargar' },
    'nav.docs': { en: 'Docs', es: 'Documentación' },
    'nav.pricing': { en: 'Pricing', es: 'Precios' },
    'nav.contact': { en: 'Contact', es: 'Contacto' },

    // ═══ Hero ═══
    'hero.badge': { en: 'Desktop Available', es: 'Desktop Disponible' },
    'hero.title1': { en: 'Git Governance and', es: 'Gobernanza y trazabilidad' },
    'hero.title2': { en: 'Operational Traceability', es: 'operativa de Git' },
    'hero.subtitle': {
        en: 'Full traceability from commit to CI to compliance. One platform for engineering teams that take operational evidence seriously.',
        es: 'Trazabilidad completa desde el commit hasta CI hasta compliance. Una plataforma para equipos de ingeniería que toman en serio la evidencia operativa.',
    },
    'hero.cta': { en: 'Download Desktop', es: 'Descargar Desktop' },
    'hero.ctaSecondary': { en: 'Explore Docs', es: 'Explorar Docs' },
    'hero.stat.traceability': { en: 'Commit Traceability', es: 'Trazabilidad de Commits' },
    'hero.stat.full': { en: 'Full', es: 'Completa' },
    'hero.stat.correlation': { en: 'CI Correlation', es: 'Correlación CI' },
    'hero.stat.audit': { en: 'Audit Trail', es: 'Pista de Auditoría' },
    'hero.stat.immutable': { en: 'Immutable', es: 'Inmutable' },

    // ═══ What is GitGov ═══
    'whatIs.badge': { en: 'What is GitGov', es: 'Qué es GitGov' },
    'whatIs.title': { en: 'Governance at the', es: 'Gobernanza en el' },
    'whatIs.titleAccent': { en: 'Source', es: 'Origen' },
    'whatIs.description': {
        en: 'GitGov is a distributed governance system that connects every Git commit to its CI pipeline, Jira ticket, and compliance audit trail — giving CTOs, CISOs, and engineering managers the visibility they need.',
        es: 'GitGov es un sistema de gobernanza distribuido que conecta cada commit de Git con su pipeline CI, ticket de Jira y pista de auditoría de compliance — dando a CTOs, CISOs y gerentes de ingeniería la visibilidad que necesitan.',
    },
    'whatIs.problemTitle': { en: 'The Problem', es: 'El Problema' },
    'whatIs.problemDescription': {
        en: 'Engineering teams ship code without a clear audit trail. Commits happen, pipelines run, tickets close — but nobody can trace the full chain of evidence when compliance asks.',
        es: 'Los equipos de ingeniería envían código sin una pista de auditoría clara. Los commits ocurren, los pipelines se ejecutan, los tickets se cierran — pero nadie puede rastrear la cadena completa de evidencia cuando compliance pregunta.',
    },
    'whatIs.solutionTitle': { en: 'The Solution', es: 'La Solución' },
    'whatIs.solutionDescription': {
        en: "GitGov captures every operation at the source — the developer's machine — and correlates it through your CI and project management tools, creating an immutable record of execution.",
        es: 'GitGov captura cada operación en el origen — la máquina del desarrollador — y la correlaciona a través de tus herramientas de CI y gestión de proyectos, creando un registro inmutable de ejecución.',
    },

    // ═══ How It Works ═══
    'howItWorks.badge': { en: 'How It Works', es: 'Cómo Funciona' },
    'howItWorks.title': { en: 'From Commit to', es: 'Del Commit al' },
    'howItWorks.titleAccent': { en: 'Compliance', es: 'Cumplimiento' },
    'howItWorks.description': {
        en: 'Three layers working together to capture, centralize, and correlate every engineering action.',
        es: 'Tres capas trabajando juntas para capturar, centralizar y correlacionar cada acción de ingeniería.',
    },
    'howItWorks.desktop': { en: 'Desktop App', es: 'App Desktop' },
    'howItWorks.desktopDesc': {
        en: "Capture every Git operation at the developer's machine",
        es: 'Captura cada operación Git en la máquina del desarrollador',
    },
    'howItWorks.controlPlane': { en: 'Control Plane', es: 'Control Plane' },
    'howItWorks.controlPlaneDesc': {
        en: 'Centralize events, enforce policies, generate audit trails',
        es: 'Centraliza eventos, aplica políticas, genera pistas de auditoría',
    },
    'howItWorks.integrations': { en: 'Integrations', es: 'Integraciones' },
    'howItWorks.integrationsDesc': {
        en: 'Correlate with Jenkins CI, Jira tickets, GitHub webhooks',
        es: 'Correlaciona con Jenkins CI, tickets de Jira, webhooks de GitHub',
    },

    // ═══ Capabilities ═══
    'capabilities.badge': { en: 'Capabilities', es: 'Capacidades' },
    'capabilities.title': { en: 'Built for', es: 'Construido para' },
    'capabilities.titleAccent': { en: 'Operational Evidence', es: 'Evidencia Operativa' },
    'capabilities.description': {
        en: 'Every feature is designed to answer one question: can you prove what happened, and when?',
        es: 'Cada funcionalidad está diseñada para responder una pregunta: ¿puedes probar qué sucedió y cuándo?',
    },
    'capabilities.governance.title': { en: 'Git Operation Governance', es: 'Gobernanza de Operaciones Git' },
    'capabilities.governance.desc': {
        en: 'Capture commits, pushes, merges, and rebases at the developer workstation level. No gaps.',
        es: 'Captura commits, pushes, merges y rebases a nivel de la estación del desarrollador. Sin vacíos.',
    },
    'capabilities.audit.title': { en: 'Immutable Audit Trail', es: 'Pista de Auditoría Inmutable' },
    'capabilities.audit.desc': {
        en: 'Append-only event logs with deduplication. Every action recorded, nothing overwritten.',
        es: 'Logs de eventos solo-agregar con deduplicación. Cada acción registrada, nada sobreescrito.',
    },
    'capabilities.ci.title': { en: 'CI Pipeline Correlation', es: 'Correlación de Pipeline CI' },
    'capabilities.ci.desc': {
        en: 'Correlate each commit with its Jenkins pipeline execution, build status, and timing.',
        es: 'Correlaciona cada commit con su ejecución de pipeline Jenkins, estado de build y timing.',
    },
    'capabilities.ticket.title': { en: 'Ticket Traceability', es: 'Trazabilidad de Tickets' },
    'capabilities.ticket.desc': {
        en: 'Map commits and CI runs to Jira tickets for complete coverage visibility.',
        es: 'Mapea commits y ejecuciones CI a tickets de Jira para visibilidad completa de cobertura.',
    },

    // ═══ Roles ═══
    'roles.badge': { en: 'Built for your role', es: 'Construido para tu rol' },
    'roles.title': { en: 'Governance for', es: 'Gobernanza para' },
    'roles.titleAccent': { en: 'Every Stakeholder', es: 'Cada Stakeholder' },
    'roles.description': {
        en: 'Different roles, same need: knowing exactly what happened in your engineering pipeline.',
        es: 'Diferentes roles, misma necesidad: saber exactamente qué sucedió en tu pipeline de ingeniería.',
    },
    'roles.cto.role': { en: 'CTO / CISO', es: 'CTO / CISO' },
    'roles.cto.pain': {
        en: 'No single source of truth for engineering activity when audits or incidents happen.',
        es: 'Sin fuente única de verdad para la actividad de ingeniería cuando ocurren auditorías o incidentes.',
    },
    'roles.cto.solution': {
        en: 'Complete audit trail from Git to CI to tickets. Evidence on demand, no manual collection.',
        es: 'Pista de auditoría completa de Git a CI a tickets. Evidencia bajo demanda, sin recolección manual.',
    },
    'roles.em.role': { en: 'Engineering Manager', es: 'Gerente de Ingeniería' },
    'roles.em.pain': {
        en: 'Fragmented visibility across Git, Jenkins, and Jira. Impossible to correlate at scale.',
        es: 'Visibilidad fragmentada entre Git, Jenkins y Jira. Imposible de correlacionar a escala.',
    },
    'roles.em.solution': {
        en: 'Automated correlation of commits → builds → tickets. See execution flow in one place.',
        es: 'Correlación automatizada de commits → builds → tickets. Ve el flujo de ejecución en un solo lugar.',
    },
    'roles.devops.role': { en: 'DevOps / Platform', es: 'DevOps / Plataforma' },
    'roles.devops.pain': {
        en: 'Policy enforcement relies on manual reviews and tribal knowledge.',
        es: 'La aplicación de políticas depende de revisiones manuales y conocimiento tribal.',
    },
    'roles.devops.solution': {
        en: 'Advisory policy checks today, with a clear path to automated enforcement.',
        es: 'Verificaciones de políticas consultivas hoy, con un camino claro hacia la aplicación automatizada.',
    },

    // ═══ CTA ═══
    'cta.title': { en: 'Ready to govern your', es: '¿Listo para gobernar tu' },
    'cta.titleAccent': { en: 'Git workflow?', es: 'flujo de trabajo Git?' },
    'cta.description': {
        en: 'Download the Desktop app and start capturing operational evidence in minutes.',
        es: 'Descarga la app Desktop y empieza a capturar evidencia operativa en minutos.',
    },
    'cta.primary': { en: 'Download Desktop', es: 'Descargar Desktop' },
    'cta.secondary': { en: 'Read the Docs', es: 'Leer la Documentación' },

    // ═══ Features Page ═══
    'features.badge': { en: 'Features', es: 'Características' },
    'features.title': { en: 'Everything you need for', es: 'Todo lo que necesitas para' },
    'features.titleAccent': { en: 'Git Governance', es: 'Gobernanza Git' },
    'features.description': {
        en: 'From commit capture to compliance reporting — every feature built around operational evidence.',
        es: 'Desde la captura de commits hasta reportes de compliance — cada funcionalidad construida alrededor de evidencia operativa.',
    },
    'features.core.badge': { en: 'Core', es: 'Core' },
    'features.core.title': { en: 'Git Operations', es: 'Operaciones Git' },
    'features.core.titleAccent': { en: 'Governance', es: 'Gobernanza' },
    'features.core.description': {
        en: "Everything starts at the developer's workstation. GitGov Desktop captures every Git operation as it happens.",
        es: 'Todo comienza en la estación del desarrollador. GitGov Desktop captura cada operación Git según sucede.',
    },
    'features.commit.title': { en: 'Commit Capture', es: 'Captura de Commits' },
    'features.commit.desc': {
        en: 'Every commit, push, merge, and rebase is recorded with metadata including author, timestamp, branch, and message hash.',
        es: 'Cada commit, push, merge y rebase se registra con metadatos incluyendo autor, timestamp, branch y hash del mensaje.',
    },
    'features.appendOnly.title': { en: 'Append-Only Storage', es: 'Almacenamiento Solo-Agregar' },
    'features.appendOnly.desc': {
        en: 'Events are stored in an append-only log with deduplication. Once recorded, nothing can be changed or deleted.',
        es: 'Los eventos se almacenan en un log solo-agregar con deduplicación. Una vez registrado, nada puede cambiarse o eliminarse.',
    },
    'features.policy.title': { en: 'Policy Checks', es: 'Verificaciones de Políticas' },
    'features.policy.desc': {
        en: 'Advisory policy checks validate commit messages, branch naming, and workflow compliance. Enforcement mode coming soon.',
        es: 'Las verificaciones consultivas validan mensajes de commit, nombrado de branches y cumplimiento del flujo de trabajo. Modo de aplicación próximamente.',
    },
    'features.infra.badge': { en: 'Infrastructure', es: 'Infraestructura' },
    'features.infra.title': { en: 'Control Plane', es: 'Control Plane' },
    'features.infra.titleAccent': { en: 'Audit Logs', es: 'Logs de Auditoría' },
    'features.infra.description': {
        en: 'Your centralized hub for all engineering governance events.',
        es: 'Tu hub centralizado para todos los eventos de gobernanza de ingeniería.',
    },
    'features.centralized.title': { en: 'Centralized Event Store', es: 'Almacén de Eventos Centralizado' },
    'features.centralized.desc': {
        en: 'All Desktop events are pushed to the Control Plane server (Rust/Axum) for centralized storage and querying.',
        es: 'Todos los eventos del Desktop se envían al servidor Control Plane (Rust/Axum) para almacenamiento y consulta centralizada.',
    },
    'features.realtime.title': { en: 'Real-Time Visibility', es: 'Visibilidad en Tiempo Real' },
    'features.realtime.desc': {
        en: 'See engineering activity as it happens. Filter by author, repository, branch, time range, or event type.',
        es: 'Ve la actividad de ingeniería según sucede. Filtra por autor, repositorio, branch, rango de tiempo o tipo de evento.',
    },
    'features.integrations.badge': { en: 'Integrations', es: 'Integraciones' },
    'features.integrations.title': { en: 'CI & Project', es: 'CI & Proyecto' },
    'features.integrations.titleAccent': { en: 'Traceability', es: 'Trazabilidad' },
    'features.integrations.description': {
        en: 'Connect the dots between commits, builds, and tickets.',
        es: 'Conecta los puntos entre commits, builds y tickets.',
    },
    'features.jenkins.title': { en: 'Jenkins Pipeline Health', es: 'Estado de Pipeline Jenkins' },
    'features.jenkins.desc': {
        en: 'Correlate commits with CI pipeline executions. See which commit triggered which build, its status, and duration.',
        es: 'Correlaciona commits con ejecuciones de pipeline CI. Ve qué commit desencadenó qué build, su estado y duración.',
    },
    'features.jira.title': { en: 'Jira Ticket Coverage', es: 'Cobertura de Tickets Jira' },
    'features.jira.desc': {
        en: "Map commits and CI runs to Jira tickets. Identify untraceable changes that aren't linked to any ticket.",
        es: 'Mapea commits y ejecuciones CI a tickets de Jira. Identifica cambios no rastreables que no están vinculados a ningún ticket.',
    },
    'features.offline.title': { en: 'Offline Resilience', es: 'Resiliencia Offline' },
    'features.offline.desc': {
        en: 'Events are queued locally when the server is unreachable. Automatic retry with exponential backoff ensures zero event loss.',
        es: 'Los eventos se encolan localmente cuando el servidor no está disponible. El reintento automático con backoff exponencial garantiza cero pérdida de eventos.',
    },
    'features.dashboard.title': { en: 'Admin Dashboard', es: 'Panel de Administración' },
    'features.dashboard.desc': {
        en: 'Built-in dashboard with recent commits table, Jenkins Pipeline Health widget (7-day view), Jira ticket badges, and 30-second auto-refresh.',
        es: 'Panel integrado con tabla de commits recientes, widget de salud de pipeline Jenkins (vista 7 días), badges de tickets Jira y auto-actualización cada 30 segundos.',
    },
    'features.github.title': { en: 'GitHub Webhooks', es: 'Webhooks de GitHub' },
    'features.github.desc': {
        en: 'Receive and process GitHub events for additional context. Push events, pull requests, reviews, and status checks.',
        es: 'Recibe y procesa eventos de GitHub para contexto adicional. Push, pull requests, reviews y verificaciones de estado.',
    },
    'features.cta.title': { en: 'See it in', es: 'Verlo en' },
    'features.cta.titleAccent': { en: 'Action', es: 'Acción' },
    'features.cta.desc': {
        en: 'Download the Desktop app and connect to your Control Plane to start capturing evidence.',
        es: 'Descarga la app Desktop y conéctate a tu Control Plane para empezar a capturar evidencia.',
    },
    'features.cta.primary': { en: 'Download Desktop', es: 'Descargar Desktop' },
    'features.cta.secondary': { en: 'Read Documentation', es: 'Leer Documentación' },

    // ═══ Download Page ═══
    'download.badge': { en: 'Download', es: 'Descargar' },
    'download.title': { en: 'Get', es: 'Obtén' },
    'download.titleAccent': { en: 'GitGov Desktop', es: 'GitGov Desktop' },
    'download.description': {
        en: 'Start capturing Git operations on your machine. Free for development teams.',
        es: 'Empieza a capturar operaciones Git en tu máquina. Gratis para equipos de desarrollo.',
    },
    'download.button': { en: 'Download .exe', es: 'Descargar .exe' },
    'download.otherPlatforms': {
        en: 'macOS and Linux builds are planned for future releases.',
        es: 'Los builds de macOS y Linux están planeados para versiones futuras.',
    },
    'download.planned': { en: 'Planned', es: 'Planeado' },
    'download.notice': {
        en: 'Build available internally. Contact the team for access.',
        es: 'Build disponible internamente. Contacta al equipo para acceso.',
    },
    'download.installNotes': { en: 'Installation Notes', es: 'Notas de Instalación' },
    'download.step1': {
        en: 'Download the <code>.exe</code> installer',
        es: 'Descarga el instalador <code>.exe</code>',
    },
    'download.step2': {
        en: 'Run the installer. If Windows displays a security verification prompt, follow the on-screen steps.',
        es: 'Ejecuta el instalador. Si Windows muestra una verificación de seguridad, sigue los pasos en pantalla.',
    },
    'download.step3': {
        en: 'Launch GitGov Desktop — it connects automatically',
        es: 'Inicia GitGov Desktop — se conecta automáticamente',
    },
    'download.step4': {
        en: 'Start working — every Git operation will be captured automatically',
        es: 'Empieza a trabajar — cada operación Git será capturada automáticamente',
    },

    'download.file': { en: 'File', es: 'Archivo' },
    'download.checksum': { en: 'Integrity (SHA256)', es: 'Integridad (SHA256)' },
    'download.copyChecksum': { en: 'Copy SHA256', es: 'Copiar SHA256' },
    'download.copiedChecksum': { en: 'Copied', es: 'Copiado' },
    'download.buttonMsi': { en: 'Download .msi', es: 'Descargar .msi' },
    'download.unsignedBanner': {
        en: 'Official GitGov installer (v0.1.0). If Windows shows a security verification prompt, follow the installation notes below.',
        es: 'Instalador oficial de GitGov (v0.1.0). Si Windows muestra una verificación de seguridad, sigue las notas de instalación de abajo.',
    },
    'download.verifyHash.title': {
        en: 'Optional integrity check (SHA256) on Windows',
        es: 'Verificación opcional de integridad (SHA256) en Windows',
    },
    'download.verifyHash.command': {
        en: 'Run in PowerShell:',
        es: 'Ejecuta en PowerShell:',
    },
    'download.verifyHash.example': {
        en: 'Expected output (Hash field):',
        es: 'Salida esperada (campo Hash):',
    },
    'download.side.heading': {
        en: 'Everything runs on your workstation',
        es: 'Todo corre en tu estación de trabajo',
    },
    'download.side.intro': {
        en: 'GitGov Desktop is a lightweight native app that captures Git events locally and syncs them to your Control Plane — no cloud dependency required.',
        es: 'GitGov Desktop es una app nativa ligera que captura eventos Git localmente y los sincroniza con tu Control Plane — sin dependencia de la nube.',
    },
    'download.side.h1title': { en: 'Git Event Capture', es: 'Captura de Eventos Git' },
    'download.side.h1desc': {
        en: 'Commits, pushes, stages, and merges captured automatically at the workstation level.',
        es: 'Commits, pushes, stages y merges capturados automáticamente a nivel de estación de trabajo.',
    },
    'download.side.h2title': { en: 'Offline Resilience', es: 'Resiliencia Offline' },
    'download.side.h2desc': {
        en: 'Local outbox queues events when the server is unreachable. Syncs automatically on reconnection.',
        es: 'La bandeja de salida local encola eventos cuando el servidor no está disponible. Sincroniza automáticamente al reconectarse.',
    },
    'download.side.h3title': { en: 'Control Plane Ready', es: 'Control Plane Listo' },
    'download.side.h3desc': {
        en: 'Connect to your self-hosted server and access the full governance dashboard instantly.',
        es: 'Conéctate a tu servidor self-hosted y accede al dashboard de gobernanza completo de inmediato.',
    },
    'download.side.h4title': { en: 'Policy Enforcement', es: 'Aplicación de Políticas' },
    'download.side.h4desc': {
        en: 'Configure gitgov.toml in your repo to enforce commit message formats and branch policies.',
        es: 'Configura gitgov.toml en tu repo para aplicar formatos de mensaje y políticas de rama.',
    },
    'download.side.sysreq': {
        en: 'Windows 10 / 11 · x64 · ~15 MB · no runtime dependencies',
        es: 'Windows 10 / 11 · x64 · ~15 MB · sin dependencias de runtime',
    },

    // ═══ Contact Page ═══
    'contact.badge': { en: 'Contact', es: 'Contacto' },
    'contact.title': { en: 'Get in', es: 'Ponte en' },
    'contact.titleAccent': { en: 'Touch', es: 'Contacto' },
    'contact.description': {
        en: "Have questions about GitGov? Want to discuss enterprise deployment? We'd love to hear from you.",
        es: '¿Tienes preguntas sobre GitGov? ¿Quieres discutir un despliegue empresarial? Nos encantaría escucharte.',
    },
    'contact.form.title': { en: 'Send us a message', es: 'Envíanos un mensaje' },
    'contact.form.subtitle': {
        en: 'All fields except company are required',
        es: 'Todos los campos excepto empresa son requeridos',
    },
    'contact.form.name': { en: 'Name', es: 'Nombre' },
    'contact.form.namePlaceholder': { en: 'Your name', es: 'Tu nombre' },
    'contact.form.email': { en: 'Email', es: 'Correo electrónico' },
    'contact.form.emailPlaceholder': { en: 'you@company.com', es: 'tu@empresa.com' },
    'contact.form.company': { en: 'Company', es: 'Empresa' },
    'contact.form.companyPlaceholder': { en: 'Your company (optional)', es: 'Tu empresa (opcional)' },
    'contact.form.message': { en: 'Message', es: 'Mensaje' },
    'contact.form.messagePlaceholder': {
        en: 'Tell us about your governance needs...',
        es: 'Cuéntanos sobre tus necesidades de gobernanza...',
    },
    'contact.form.send': { en: 'Send Message', es: 'Enviar Mensaje' },
    'contact.form.sending': { en: 'Sending...', es: 'Enviando...' },
    'contact.success.title': { en: 'Message Sent', es: 'Mensaje Enviado' },
    'contact.success.description': {
        en: "Thank you for reaching out. We'll get back to you as soon as possible.",
        es: 'Gracias por contactarnos. Te responderemos lo antes posible.',
    },
    'contact.success.button': { en: 'Send another message', es: 'Enviar otro mensaje' },
    'contact.error': {
        en: 'Something went wrong. Please try again.',
        es: 'Algo salió mal. Por favor inténtalo de nuevo.',
    },
    'contact.errors.name': { en: 'Name is required', es: 'El nombre es requerido' },
    'contact.errors.email': { en: 'Email is required', es: 'El correo es requerido' },
    'contact.errors.emailInvalid': { en: 'Invalid email address', es: 'Correo electrónico inválido' },
    'contact.errors.message': { en: 'Message is required', es: 'El mensaje es requerido' },
    'contact.side.heading': { en: "Let's talk about your team", es: 'Hablemos de tu equipo' },
    'contact.side.intro': {
        en: "We help engineering teams enforce governance policies without slowing down development. Tell us about your stack and we'll get back to you shortly.",
        es: 'Ayudamos a los equipos de ingeniería a aplicar políticas de gobernanza sin frenar el desarrollo. Cuéntanos sobre tu stack y te responderemos pronto.',
    },
    'contact.side.responseTime': {
        en: 'We typically respond within 1 business day.',
        es: 'Solemos responder en menos de 1 día hábil.',
    },
    'contact.side.h1title': { en: 'Enterprise-grade security', es: 'Seguridad empresarial' },
    'contact.side.h1desc': { en: 'Append-only audit logs and SHA256-hashed API keys out of the box.', es: 'Logs de auditoría inmutables y API keys hasheadas con SHA256 desde el primer día.' },
    'contact.side.h2title': { en: 'Fast deployment', es: 'Despliegue rápido' },
    'contact.side.h2desc': { en: 'Up and running in minutes with your existing Git workflow.', es: 'Funcionando en minutos con tu flujo de trabajo Git existente.' },
    'contact.side.h3title': { en: 'Dedicated support', es: 'Soporte dedicado' },
    'contact.side.h3desc': { en: 'Priority support and custom onboarding for enterprise plans.', es: 'Soporte prioritario y onboarding personalizado para planes Enterprise.' },
    'contact.side.h4title': { en: 'GitHub, Jenkins & Jira', es: 'GitHub, Jenkins y Jira' },
    'contact.side.h4desc': { en: 'Native integrations with the tools your team already uses.', es: 'Integraciones nativas con las herramientas que tu equipo ya usa.' },

    // ═══ Pricing Page ═══
    'pricing.badge': { en: 'Pricing', es: 'Precios' },
    'pricing.title': { en: 'Plans &', es: 'Planes y' },
    'pricing.titleAccent': { en: 'Pricing', es: 'Precios' },
    'pricing.description': {
        en: "We're finalizing our pricing model. Sign up to be notified when plans are available.",
        es: 'Estamos finalizando nuestro modelo de precios. Regístrate para ser notificado cuando los planes estén disponibles.',
    },
    'pricing.comingSoon': { en: 'Coming Soon', es: 'Próximamente' },
    'pricing.underDev': { en: 'Pricing Under Development', es: 'Precios en Desarrollo' },
    'pricing.underDevDesc': {
        en: "GitGov is currently available for internal teams. We're working on Enterprise and Team plans with transparent pricing. In the meantime, reach out directly to discuss your needs.",
        es: 'GitGov está actualmente disponible para equipos internos. Estamos trabajando en planes Enterprise y Team con precios transparentes. Mientras tanto, contáctanos directamente para discutir tus necesidades.',
    },
    'pricing.contactBtn': { en: 'Contact for Pricing', es: 'Contactar por Precios' },
    'pricing.features': {
        en: ['Unlimited Git operation capture', 'Control Plane access', 'Jenkins CI correlation', 'Jira ticket coverage', 'Append-only audit logs', 'Policy advisory checks', 'Team management', 'Priority support'],
        es: ['Captura ilimitada de operaciones Git', 'Acceso al Control Plane', 'Correlación Jenkins CI', 'Cobertura de tickets Jira', 'Logs de auditoría inmutables', 'Verificaciones de políticas consultivas', 'Gestión de equipos', 'Soporte prioritario'],
    },
    'pricing.descriptionNew': {
        en: 'Simple, transparent pricing for teams of every size. All plans include core governance features.',
        es: 'Precios simples y transparentes para equipos de cualquier tamaño. Todos los planes incluyen las funciones de gobernanza principales.',
    },
    'pricing.bottomNote': {
        en: 'All plans are currently in development. Contact us to discuss your team\'s needs.',
        es: 'Todos los planes están actualmente en desarrollo. Contáctanos para discutir las necesidades de tu equipo.',
    },
    // Free plan
    'pricing.plans.free.name': { en: 'Starter', es: 'Starter' },
    'pricing.plans.free.price': { en: 'Free', es: 'Gratis' },
    'pricing.plans.free.priceNote': { en: 'For small teams & evaluation', es: 'Para equipos pequeños y evaluación' },
    'pricing.plans.free.description': { en: 'Get started with Git governance for your team.', es: 'Comienza con la gobernanza Git para tu equipo.' },
    'pricing.plans.free.cta': { en: 'Get Started', es: 'Empezar' },
    'pricing.plans.free.f1': { en: 'Git operation capture', es: 'Captura de operaciones Git' },
    'pricing.plans.free.f2': { en: 'Basic audit logs', es: 'Logs de auditoría básicos' },
    'pricing.plans.free.f3': { en: 'Up to 5 users', es: 'Hasta 5 usuarios' },
    'pricing.plans.free.f4': { en: 'Jenkins CI correlation', es: 'Correlación Jenkins CI' },
    'pricing.plans.free.f5': { en: 'Jira ticket coverage', es: 'Cobertura de tickets Jira' },
    'pricing.plans.free.f6': { en: 'Compliance reports', es: 'Informes de cumplimiento' },
    // Team plan
    'pricing.plans.team.name': { en: 'Team', es: 'Team' },
    'pricing.plans.team.badge': { en: 'Most Popular', es: 'Más Popular' },
    'pricing.plans.team.price': { en: 'Contact us', es: 'Contáctanos' },
    'pricing.plans.team.priceNote': { en: 'Per team · custom pricing', es: 'Por equipo · precio personalizado' },
    'pricing.plans.team.description': { en: 'Full governance for growing engineering teams.', es: 'Gobernanza completa para equipos de ingeniería en crecimiento.' },
    'pricing.plans.team.cta': { en: 'Contact for Pricing', es: 'Contactar por Precios' },
    'pricing.plans.team.f1': { en: 'Everything in Starter', es: 'Todo en Starter' },
    'pricing.plans.team.f2': { en: 'Append-only audit logs', es: 'Logs de auditoría inmutables' },
    'pricing.plans.team.f3': { en: 'Jenkins CI correlation', es: 'Correlación Jenkins CI' },
    'pricing.plans.team.f4': { en: 'Jira ticket coverage', es: 'Cobertura de tickets Jira' },
    'pricing.plans.team.f5': { en: 'Policy advisory checks', es: 'Verificaciones de políticas' },
    'pricing.plans.team.f6': { en: 'Compliance reports', es: 'Informes de cumplimiento' },
    // Enterprise plan
    'pricing.plans.enterprise.name': { en: 'Enterprise', es: 'Enterprise' },
    'pricing.plans.enterprise.price': { en: 'Custom', es: 'Personalizado' },
    'pricing.plans.enterprise.priceNote': { en: 'Tailored to your organization', es: 'Adaptado a tu organización' },
    'pricing.plans.enterprise.description': { en: 'Full control, compliance & dedicated support at scale.', es: 'Control total, cumplimiento y soporte dedicado a escala.' },
    'pricing.plans.enterprise.cta': { en: 'Talk to Sales', es: 'Hablar con Ventas' },
    'pricing.plans.enterprise.f1': { en: 'Everything in Team', es: 'Todo en Team' },
    'pricing.plans.enterprise.f2': { en: 'Unlimited users', es: 'Usuarios ilimitados' },
    'pricing.plans.enterprise.f3': { en: 'Compliance reports (SSO roadmap)', es: 'Informes de cumplimiento (SSO en roadmap)' },
    'pricing.plans.enterprise.f4': { en: 'Custom integrations', es: 'Integraciones personalizadas' },
    'pricing.plans.enterprise.f5': { en: 'SLA & priority support', es: 'SLA y soporte prioritario' },
    'pricing.plans.enterprise.f6': { en: 'Dedicated onboarding', es: 'Onboarding dedicado' },

    // ═══ 404 ═══
    '404.title': { en: 'Page Not Found', es: 'Página No Encontrada' },
    '404.description': {
        en: "The page you're looking for doesn't exist or has been moved. Check the URL or head back to a known route.",
        es: 'La página que buscas no existe o ha sido movida. Revisa la URL o regresa a una ruta conocida.',
    },
    '404.home': { en: 'Back to Home', es: 'Volver al Inicio' },
    '404.docs': { en: 'Browse Docs', es: 'Explorar Docs' },

    // ═══ Navigation extra ═══
    'nav.privacy': { en: 'Privacy Policy', es: 'Política de Privacidad' },

    // ═══ Footer ═══
    'footer.product': { en: 'Product', es: 'Producto' },
    'footer.resources': { en: 'Resources', es: 'Recursos' },
    'footer.resources.documentation': { en: 'Documentation', es: 'Documentación' },
    'footer.resources.installationguide': { en: 'Installation Guide', es: 'Guía de Instalación' },
    'footer.resources.controlplanesetup': { en: 'Control Plane Setup', es: 'Configuración Control Plane' },
    'footer.company': { en: 'Company', es: 'Empresa' },
    'footer.rights': { en: 'All rights reserved.', es: 'Todos los derechos reservados.' },
    'footer.tagline': { en: 'Governance · Traceability · Compliance', es: 'Gobernanza · Trazabilidad · Cumplimiento' },

    // ═══ Docs ═══
    'docs.title': { en: 'Documentation', es: 'Documentación' },

    // ═══ Misc ═══
    'advisory': { en: 'Advisory', es: 'Consultivo' },
    'preview': { en: 'Preview', es: 'Vista Previa' },
    'inProgress': { en: 'In Progress', es: 'En Progreso' },
    'available': { en: 'Available', es: 'Disponible' },
    'challenge': { en: 'Challenge', es: 'Desafío' },
    'withGitGov': { en: 'With GitGov', es: 'Con GitGov' },
} as const;

export type TranslationKey = keyof typeof translations;
