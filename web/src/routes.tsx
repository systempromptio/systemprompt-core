import type { RouteObject } from 'react-router-dom';
import App from './App';
import { ErrorFallback } from './components/ErrorFallback';

export const routes: RouteObject[] = [
  {
    path: '/',
    element: <App />,
    errorElement: <ErrorFallback />,
  },
];
