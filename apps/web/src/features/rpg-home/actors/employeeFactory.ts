import { EmployeeRecord, RpgActor } from '../types'
import { WORKSTATION_ANCHORS } from '../scene/workstationAnchors'

export function createEmployeeActors(employees: EmployeeRecord[]): RpgActor[] {
  return employees.map((employee, index) => {
    const anchor = WORKSTATION_ANCHORS[index % WORKSTATION_ANCHORS.length]
    return {
      id: `employee-${employee.id}`,
      kind: 'employee',
      name: employee.name,
      department: employee.department,
      role: employee.role,
      position: anchor,
    }
  })
}
