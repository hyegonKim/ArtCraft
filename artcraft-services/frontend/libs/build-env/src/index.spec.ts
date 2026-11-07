import { describe, expect, it } from 'vitest';
import { BuildEnv, BuildEnvType, GetBuildEnv, GetBuildEnvType } from './index';

describe('build-env public entry point', () => {
  it('exports the environment singleton and environment types', () => {
    expect(GetBuildEnv()).toBeInstanceOf(BuildEnv);
    expect(GetBuildEnv()).toBe(BuildEnv.getInstance());
    expect(GetBuildEnv().getType()).toBe(GetBuildEnvType());
    expect(Object.values(BuildEnvType)).toContain(GetBuildEnvType());
  });
});
