
// Flags that can be toggled from the dev console via window.setDevFlag.

export const SetDevFlag = (flagName: string, value: any) => {
  if (!!!(window as any)._dev_flags) {
    (window as any)._dev_flags = {};
  }
  (window as any)._dev_flags[flagName] = value;
}

export const GetDevFlag = (flagName: string) => {
  if (!!(window as any)._dev_flags && flagName in (window as any)._dev_flags) {
    return (window as any)._dev_flags[flagName];
  }
  return null;
}

(window as any).setDevFlag = SetDevFlag;
(window as any).getDevFlag = GetDevFlag;


