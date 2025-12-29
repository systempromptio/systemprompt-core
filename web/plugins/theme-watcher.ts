import type { Plugin } from 'vite';
import { readFileSync } from 'fs';
import { join } from 'path';
import { execSync } from 'child_process';

export function themeWatcher(): Plugin {
  const yamlPath = process.env.SYSTEMPROMPT_WEB_CONFIG_PATH;
  if (!yamlPath) {
    throw new Error('SYSTEMPROMPT_WEB_CONFIG_PATH environment variable must be set');
  }

  return {
    name: 'theme-watcher',

    configureServer(server) {
      const regenerateTheme = () => {
        try {
          console.log('🎨 Regenerating theme from YAML...');
          execSync('npm run theme:generate', { stdio: 'inherit' });
          console.log('✅ Theme regenerated successfully');

          server.ws.send({
            type: 'full-reload',
            path: '*'
          });
        } catch (error) {
          console.error('❌ Failed to regenerate theme:', error);
        }
      };

      server.watcher.add(yamlPath);

      server.watcher.on('change', (file) => {
        if (file.includes('web.yaml')) {
          regenerateTheme();
        }
      });
    },

    buildStart() {
      try {
        readFileSync(join(process.cwd(), 'src/styles/theme.generated.css'), 'utf8');
      } catch (error) {
        console.warn('⚠️  theme.generated.css not found. Run npm run theme:generate first.');
      }
    }
  };
}
