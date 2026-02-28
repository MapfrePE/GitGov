# GitGov Golden Path Checklist (No Regresión)

**Objetivo:** Validar que cambios en server/auth/dashboard/integraciones no rompan el flujo base que ya funciona.

---

## Flujo Crítico (debe seguir funcionando)

1. Desktop detecta cambios de archivos
2. Desktop permite `commit` con mensaje
3. Desktop permite `push`
4. Control Plane recibe eventos (`stage_files`, `commit`, `attempt_push`, `successful_push`)
5. Dashboard muestra `Commits Recientes` sin `401`

---

## Checklist Manual (rápido)

## A. Desktop / Git local
- [ ] Abrir repo en Desktop
- [ ] Editar 1 archivo
- [ ] Ver archivo en lista de cambios
- [ ] Hacer `commit` desde la app con mensaje visible (ej: `feat: prueba golden path`)
- [ ] Hacer `push` exitoso

## B. Control Plane / Server
- [ ] `GET /health` responde `200`
- [ ] `GET /stats` con `Authorization: Bearer` responde `200`
- [ ] `GET /logs` con `Authorization: Bearer` responde `200`
- [ ] No aparece `401 Unauthorized` en dashboard

## C. Dashboard UI
- [ ] En `Commits Recientes` aparece una sola fila por commit (sin `attempt_push` / `successful_push`)
- [ ] Se ve mensaje del commit
- [ ] Se ve hash corto
- [ ] `Ver archivos (N)` despliega archivos del commit

## D. Integraciones V1.2-A (si aplican)
- [ ] `GET /integrations/jenkins/status` responde `200` (admin)
- [ ] `POST /integrations/jenkins` acepta payload válido
- [ ] Payload duplicado devuelve `duplicate=true`
- [ ] `GET /integrations/jenkins/correlations` responde `200`
- [ ] Widget `Pipeline Health (7 días)` carga (aunque muestre vacío si no hay datos)

---

## Comandos útiles

### E2E base
```bash
cd gitgov/gitgov-server/tests
./e2e_flow_test.sh
```

### Jenkins integration (V1.2-A)
```bash
cd gitgov/gitgov-server/tests
API_KEY="TU_API_KEY_ADMIN" ./jenkins_integration_test.sh
```

Con secret habilitado:
```bash
API_KEY="TU_API_KEY_ADMIN" JENKINS_SECRET="tu_secreto" ./jenkins_integration_test.sh
```

---

## E. Diagnóstico de Topología Local (anti split-brain)

Antes de cualquier sesión de desarrollo, verifica que no haya dos servidores
compitiendo en los puertos 3000/3001:

```powershell
# Desde la raíz del repo
.\scripts\check_local_topology.ps1
```

**Salida esperada (estado limpio):**
```
[OK] Solo un server activo en 3000. Sin riesgo de split-brain.
```

**Señal de alerta:**
```
[WARN] Procesos en AMBOS puertos 3000 y 3001.
       Riesgo de SPLIT-BRAIN ...
```

| Puerto | Rol canónico |
|--------|-------------|
| `127.0.0.1:3000` | Server local dev (`cargo run`) |
| `127.0.0.1:3001` | Server Docker / alternativo |

> Si aparece `[WARN]`, detén el proceso incorrecto antes de usar el dashboard.
> El script también hace `GET /health` en cada puerto para confirmar que el
> proceso que escucha es realmente un server GitGov.

---

## Regla Operativa

Si un cambio rompe cualquiera de los checks A/B/C, **se considera regresión del core** y debe corregirse antes de continuar con nuevas features.

**Antes de release:**
```bash
cd gitgov/gitgov-server
make test   # unit tests — no servidor necesario (también corre en CI)
make smoke  # contrato live — requiere servidor corriendo (cargo run)
```

> Nota: `make test` valida el contrato de payload/respuesta (structs serde, Golden Path shapes).
> `make smoke` valida el flujo real contra server+DB (no corre en CI).

