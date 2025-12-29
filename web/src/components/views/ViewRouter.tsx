import { lazy, Suspense } from 'react';
import { ErrorBoundary } from '@/components/ErrorBoundary';
import { useViewStore } from '@/stores/view.store';
import { logger } from '@/lib/logger';

const ChatInterface = lazy(() => import('@/components/chat/ChatInterface').then(m => ({ default: m.ChatInterface })));
const ArtifactsView = lazy(() => import('@/components/artifacts/ArtifactsView').then(m => ({ default: m.ArtifactsView })));

export function ViewRouter() {
  const activeView = useViewStore((state) => state.activeView);

  const renderView = () => {
    switch (activeView) {
      case 'conversation':
        return (
          <ErrorBoundary
            fallbackVariant="inline"
            showDetails={true}
            retryable={true}
            onError={(error) => {
              logger.error('Chat view error', error, 'ViewRouter');
            }}
          >
            <Suspense fallback={<div className="w-full py-12 flex items-center justify-center bg-surface rounded-lg border border-border"><div className="flex flex-col items-center gap-3"><span className="text-text-secondary">Loading conversation...</span></div></div>}>
              <ChatInterface />
            </Suspense>
          </ErrorBoundary>
        );

      case 'artifacts':
        return (
          <ErrorBoundary
            fallbackVariant="inline"
            showDetails={true}
            retryable={true}
            onError={(error) => {
              logger.error('Artifacts view error', error, 'ViewRouter');
            }}
          >
            <Suspense fallback={<div className="w-full py-12 flex items-center justify-center bg-surface rounded-lg border border-border"><div className="flex flex-col items-center gap-3"><span className="text-text-secondary">Loading artifacts...</span></div></div>}>
              <ArtifactsView />
            </Suspense>
          </ErrorBoundary>
        );

      default:
        return (
          <ErrorBoundary fallbackVariant="inline" showDetails={true} retryable={true}>
            <Suspense fallback={<div className="w-full py-12 flex items-center justify-center bg-surface rounded-lg border border-border"><div className="flex flex-col items-center gap-3"><span className="text-text-secondary">Loading conversation...</span></div></div>}>
              <ChatInterface />
            </Suspense>
          </ErrorBoundary>
        );
    }
  };

  return renderView();
}
