import { useState } from 'react'
import { useAuthStore } from '@/store/useAuthStore'
import { useRepoStore } from '@/store/useRepoStore'
import { useUpdateStore } from '@/store/useUpdateStore'
import { useControlPlaneStore } from '@/store/useControlPlaneStore'
import { Header } from '@/components/layout/Header'
import { Button } from '@/components/shared/Button'
import { Modal } from '@/components/shared/Modal'
import { Link } from 'react-router-dom'
import { User, FolderOpen, FileCode, LogOut, Shield, Users, Download, RefreshCw, Sparkles, ExternalLink, Globe } from 'lucide-react'
import { TIMEZONES, detectBrowserTimezone, formatTs } from '@/lib/timezone'
import { AdminOnboardingPanel } from '@/components/control_plane/AdminOnboardingPanel'
import { TeamManagementPanel } from '@/components/control_plane/TeamManagementPanel'
import { ApiKeyManagerWidget } from '@/components/control_plane/ApiKeyManagerWidget'

export function SettingsPage() {
  const { user, logout, isPinEnabled, setLocalPin, clearLocalPin, lockSession, pinError } = useAuthStore()
  const { repoPath, config } = useRepoStore()
  const { displayTimezone, setDisplayTimezone, isConnected, userRole } = useControlPlaneStore()
  const {
    status: updaterStatus,
    isChecking,
    isDownloading,
    isUpdaterSupported,
    isUpdaterConfigured,
    updateInfo,
    progress,
    lastCheckedAt,
    error: updaterError,
    channel: updateChannel,
    fallbackDownloadUrl,
    changelogExpanded,
    telemetry: updaterTelemetry,
    checkForUpdates,
    downloadAndInstall,
    retryDownload,
    setChannel,
    setChangelogExpanded,
  } = useUpdateStore()
  const [showRepoSelector, setShowRepoSelector] = useState(false)
  const [pinInput, setPinInput] = useState('')
  const isControlPlaneAdmin = userRole === 'Admin'
  const canManageOrgSettings = Boolean(user?.is_admin) || isControlPlaneAdmin

  return (
    <div className="h-full flex flex-col bg-surface-950">
      <Header />

      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-6xl mx-auto space-y-5 animate-fade-in">
          <section className="rounded-2xl border border-surface-700/30 bg-surface-800/40 p-6">
            <div className="card-header mb-2">
              <Globe size={12} strokeWidth={1.5} />
              Zona Horaria del Audit Trail
            </div>
            <p className="text-xs text-surface-400 mb-4">
              Los eventos se almacenan en UTC en la base de datos. Selecciona la zona horaria local para mostrar los timestamps correctamente en auditorías legales.
            </p>
            <div className="space-y-3">
              <div className="rounded-lg border border-surface-700/30 bg-surface-900/50 p-3 space-y-2">
                <p className="text-[10px] text-surface-500 uppercase tracking-widest font-medium">Zona horaria de visualización</p>
                <select
                  value={displayTimezone}
                  onChange={(e) => setDisplayTimezone(e.target.value)}
                  className="w-full bg-surface-900 border border-surface-700/50 rounded-lg px-3 py-2 text-xs text-surface-100 focus:outline-none focus:border-brand-500/60"
                >
                  {TIMEZONES.map((tz) => (
                    <option key={tz.value} value={tz.value}>{tz.label}</option>
                  ))}
                  {!TIMEZONES.some((tz) => tz.value === displayTimezone) && (
                    <option value={displayTimezone}>{displayTimezone}</option>
                  )}
                </select>
                <div className="flex items-center gap-2 flex-wrap">
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => setDisplayTimezone(detectBrowserTimezone())}
                  >
                    Auto-detectar del sistema
                  </Button>
                  <span className="text-[10px] text-surface-500">
                    Activa: <span className="text-surface-300 font-medium font-mono">{displayTimezone}</span>
                  </span>
                </div>
              </div>
            </div>
          </section>

          {canManageOrgSettings && (
            <section className="rounded-2xl border border-surface-700/30 bg-surface-800/40 p-6">
              <div className="card-header mb-2">Administración de Organización</div>
              <p className="text-xs text-surface-400 mb-4">
                Onboarding admin, gestión de equipo y API keys se gestionan desde Settings.
                Export JSON se mantiene fuera de esta vista.
              </p>

              {!isConnected ? (
                <div className="rounded-lg border border-white/8 bg-surface-900/50 p-4 text-xs text-surface-300">
                  Conecta primero al Control Plane para administrar organización y acceso por rol.
                  <div className="mt-3">
                    <Link to="/control-plane">
                      <Button size="sm" variant="secondary">Abrir Control Plane</Button>
                    </Link>
                  </div>
                </div>
              ) : (
                <div className="space-y-4">
                  <AdminOnboardingPanel />
                  <TeamManagementPanel />
                  <ApiKeyManagerWidget />
                </div>
              )}
            </section>
          )}

          <section className="rounded-2xl border border-surface-700/30 bg-surface-800/40 p-6">
            <div className="card-header mb-5">
              <Sparkles size={12} strokeWidth={1.5} />
              Actualizaciones Desktop
            </div>

            <div className="space-y-3">
              <div className="rounded-lg border border-surface-700/30 bg-surface-900/50 p-3">
                <p className="text-[10px] text-surface-500 uppercase tracking-widest font-medium mb-2">
                  Canal de actualizaciones
                </p>
                <div className="flex flex-wrap gap-2">
                  <Button
                    size="sm"
                    variant={updateChannel === 'stable' ? 'primary' : 'secondary'}
                    onClick={() => setChannel('stable')}
                    disabled={isChecking || isDownloading}
                    title="Canal recomendado para usuarios finales"
                  >
                    Stable
                  </Button>
                  <Button
                    size="sm"
                    variant={updateChannel === 'beta' ? 'primary' : 'secondary'}
                    onClick={() => setChannel('beta')}
                    disabled={isChecking || isDownloading}
                    title="Canal beta para pruebas internas"
                  >
                    Beta
                  </Button>
                </div>
                <p className="text-[10px] text-surface-500 mt-2">
                  Canal activo: <span className="text-surface-300 font-medium">{updateChannel}</span>
                </p>
              </div>

              <div className="rounded-lg border border-surface-700/30 bg-surface-900/50 p-3">
                <p className="text-[10px] text-surface-500 uppercase tracking-widest font-medium mb-1">
                  Estado del updater
                </p>
                <p className="text-xs text-surface-200">
                  {!isUpdaterSupported
                    ? 'Updater in-app no disponible fuera de Tauri Desktop.'
                    : updaterStatus === 'not-configured'
                      ? 'Updater no configurado (faltan endpoint/pubkey firmados).'
                      : updaterStatus === 'update-available'
                        ? `Nueva versión disponible: ${updateInfo?.version ?? 'desconocida'}`
                        : updaterStatus === 'installed'
                          ? 'Update instalado. Reinicia GitGov para aplicar cambios.'
                          : updaterStatus === 'downloading'
                            ? 'Descargando actualización...'
                            : updaterStatus === 'checking'
                              ? 'Buscando actualizaciones...'
                              : updaterStatus === 'no-update'
                                ? 'GitGov está actualizado.'
                                : 'Listo para verificar actualizaciones.'}
                </p>
                {lastCheckedAt && (
                  <p className="text-[10px] text-surface-500 mt-1">
                    Última verificación: {formatTs(lastCheckedAt, displayTimezone)}
                  </p>
                )}
                <p className="text-[10px] text-surface-500 mt-1">
                  Checks: {updaterTelemetry.checks} · Con update: {updaterTelemetry.updateChecksWithUpdate} · Descargas: {updaterTelemetry.downloadAttempts} · Instaladas: {updaterTelemetry.installSuccesses} · Fallidas: {updaterTelemetry.installFailures}
                </p>
                {updaterTelemetry.lastEventAt && (
                  <p className="text-[10px] text-surface-500 mt-1">
                    Último resultado: <span className="text-surface-300">{updaterTelemetry.lastOutcome}</span> · {formatTs(updaterTelemetry.lastEventAt, displayTimezone)}
                  </p>
                )}
                {updaterError && (
                  <p className="text-[10px] text-danger-400 mt-1 wrap-break-word">{updaterError}</p>
                )}
                {!isUpdaterConfigured && isUpdaterSupported && (
                  <p className="text-[10px] text-warning-300 mt-1">
                    Configura `plugins.updater` en `tauri.conf.json` con endpoint(s) y pubkey de firma para activar updates in-app.
                  </p>
                )}
              </div>

              {updateInfo && (
                <div className="rounded-lg border border-brand-500/20 bg-brand-500/5 p-3">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <div>
                      <p className="text-sm font-semibold text-white tracking-tight">
                        v{updateInfo.version}
                      </p>
                      <p className="text-[10px] text-surface-500">
                        Actual: v{updateInfo.currentVersion}
                        {updateInfo.date ? ` · ${formatTs(Date.parse(updateInfo.date), displayTimezone)}` : ''}
                      </p>
                    </div>
                    <div className="flex flex-wrap gap-2">
                      <Button
                        variant="secondary"
                        size="sm"
                        onClick={() => setChangelogExpanded(!changelogExpanded)}
                      >
                        {changelogExpanded ? 'Ocultar changelog' : 'Ver changelog'}
                      </Button>
                      <Button
                        size="sm"
                        onClick={() => void downloadAndInstall()}
                        loading={isDownloading}
                      >
                        <Download size={13} strokeWidth={1.5} />
                        Descargar e instalar
                      </Button>
                      {updaterStatus === 'error' && (
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => void retryDownload()}
                          disabled={isDownloading}
                        >
                          <RefreshCw size={13} strokeWidth={1.5} />
                          Reintentar descarga
                        </Button>
                      )}
                    </div>
                  </div>

                  {isDownloading && (
                    <div className="mt-2">
                      <div className="h-1.5 rounded bg-surface-800 overflow-hidden">
                        <div
                          className="h-full bg-brand-500 transition-all duration-200"
                          style={{
                            width: progress?.totalBytes && progress.totalBytes > 0
                              ? `${Math.min(100, (progress.downloadedBytes / progress.totalBytes) * 100)}%`
                              : '20%',
                          }}
                        />
                      </div>
                      <p className="text-[10px] text-surface-500 mt-1">
                        {progress?.downloadedBytes
                          ? `${Math.round(progress.downloadedBytes / 1024)} KB descargados`
                          : 'Preparando descarga...'}
                      </p>
                    </div>
                  )}

                  {changelogExpanded && (
                    <div className="mt-2 rounded border border-white/6 bg-surface-950/50 p-2">
                      <p className="text-[10px] text-surface-500 mb-1">Changelog</p>
                      <pre className="text-[11px] whitespace-pre-wrap text-surface-300 leading-relaxed">
                        {updateInfo.body?.trim() || 'Sin changelog en esta release.'}
                      </pre>
                    </div>
                  )}
                </div>
              )}

              <div className="flex flex-wrap gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void checkForUpdates({ manual: true, force: true })}
                  loading={isChecking}
                >
                  <RefreshCw size={13} strokeWidth={1.5} />
                  Buscar actualizaciones
                </Button>
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => window.open(fallbackDownloadUrl, '_blank', 'noopener,noreferrer')}
                  title="Fallback si el updater no está configurado o falla"
                >
                  <ExternalLink size={13} strokeWidth={1.5} />
                  Descarga manual
                </Button>
              </div>
            </div>
          </section>

          <section className="rounded-2xl border border-surface-700/30 bg-surface-800/40 p-6">
            <div className="card-header mb-5">
              <User size={12} strokeWidth={1.5} />
              Sesión
            </div>
            {user && (
              <div className="space-y-4">
                <div className="flex items-center gap-4">
                  <img
                    src={user.avatar_url}
                    alt={user.login}
                    className="w-12 h-12 rounded-full ring-2 ring-surface-700 ring-offset-2 ring-offset-surface-800"
                  />
                  <div>
                    <p className="text-white font-semibold tracking-tight">{user.name}</p>
                    <p className="text-surface-500 text-xs">@{user.login}</p>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  {user.is_admin && (
                    <span className="text-[10px] font-medium bg-brand-500/10 text-brand-400 px-2 py-0.5 rounded inline-flex items-center gap-1">
                      <Shield size={9} />
                      Administrador
                    </span>
                  )}
                  {user.group && (
                    <span className="text-[10px] font-medium bg-surface-700/40 text-surface-400 px-2 py-0.5 rounded inline-flex items-center gap-1">
                      <Users size={9} />
                      {user.group}
                    </span>
                  )}
                </div>
                <Button variant="danger" size="sm" onClick={logout}>
                  <LogOut size={13} strokeWidth={1.5} />
                  Cerrar sesión
                </Button>
                <Button variant="secondary" size="sm" onClick={logout}>
                  Cambiar usuario
                </Button>

                <div className="mt-3 pt-3 border-t border-surface-700/30 space-y-2">
                  <p className="text-[10px] text-surface-500 uppercase tracking-widest font-medium">PIN local (opcional)</p>
                  <p className="text-[11px] text-surface-500">
                    Protege el acceso local a la app en esta máquina. No reemplaza autenticación de servidor.
                  </p>
                  <div className="flex items-center gap-2">
                    <input
                      type="password"
                      inputMode="numeric"
                      pattern="[0-9]*"
                      value={pinInput}
                      onChange={(e) => setPinInput(e.target.value)}
                      placeholder={isPinEnabled ? 'Nuevo PIN (4-6 dígitos)' : 'PIN (4-6 dígitos)'}
                      className="bg-surface-900/60 rounded-lg border border-surface-700/30 px-3 py-1.5 text-xs text-white outline-none focus:border-brand-500/60"
                    />
                    <Button
                      size="sm"
                      onClick={() => {
                        setLocalPin(pinInput)
                        setPinInput('')
                      }}
                    >
                      {isPinEnabled ? 'Actualizar PIN' : 'Activar PIN'}
                    </Button>
                    {isPinEnabled && (
                      <Button variant="outline" size="sm" onClick={clearLocalPin}>
                        Desactivar PIN
                      </Button>
                    )}
                    {isPinEnabled && (
                      <Button variant="secondary" size="sm" onClick={lockSession}>
                        Bloquear ahora
                      </Button>
                    )}
                  </div>
                  {pinError && <p className="text-[11px] text-danger-400">{pinError}</p>}
                </div>
              </div>
            )}
          </section>

          <section className="rounded-2xl border border-surface-700/30 bg-surface-800/40 p-6">
            <div className="card-header mb-5">
              <FolderOpen size={12} strokeWidth={1.5} />
              Repositorio
            </div>
            <div className="space-y-3">
              <div>
                <p className="text-[10px] text-surface-500 uppercase tracking-widest mb-1.5 font-medium">Ruta actual</p>
                <p className="text-white mono-data text-xs bg-surface-900/60 p-3 rounded-lg border border-surface-700/30">
                  {repoPath || 'No seleccionado'}
                </p>
              </div>
              <Button variant="secondary" onClick={() => setShowRepoSelector(true)}>
                <FolderOpen size={13} strokeWidth={1.5} />
                Cambiar repositorio
              </Button>
            </div>
          </section>

          {config && (
            <section className="rounded-2xl border border-surface-700/30 bg-surface-800/40 p-6">
              <div className="card-header mb-5">
                <FileCode size={12} strokeWidth={1.5} />
                Configuración GitGov
              </div>
              <div className="bg-surface-900/60 rounded-lg p-4 border border-surface-700/30">
                <pre className="text-[11px] mono-data overflow-auto whitespace-pre-wrap leading-relaxed">
                  {JSON.stringify(config, null, 2).split('\n').map((line, i) => {
                    const keyMatch = line.match(/^(\s*)"([^"]+)"(:)/)
                    if (keyMatch) {
                      return (
                        <span key={i}>
                          {keyMatch[1]}<span className="text-brand-400">"{keyMatch[2]}"</span>{keyMatch[3]}
                          <span className="text-surface-400">{line.slice(keyMatch[0].length)}</span>{'\n'}
                        </span>
                      )
                    }
                    return <span key={i} className="text-surface-400">{line}{'\n'}</span>
                  })}
                </pre>
              </div>
            </section>
          )}
        </div>
      </div>

      <Modal
        isOpen={showRepoSelector}
        onClose={() => setShowRepoSelector(false)}
        title="Cambiar repositorio"
        size="lg"
      >
        <div className="text-center py-4">
          <p className="text-surface-400 text-sm mb-4">
            Selecciona un nuevo repositorio para gestionar
          </p>
          <Button onClick={() => setShowRepoSelector(false)}>
            Seleccionar desde el selector principal
          </Button>
        </div>
      </Modal>
    </div>
  )
}
