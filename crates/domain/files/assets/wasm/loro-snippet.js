
const kinds = ['Map', 'List', 'MovableList', 'Text', 'Tree', 'Counter'];
export function stateOptions(options, document) {
  if (typeof options !== 'object' || options === null || Array.isArray(options)) throw new Error('Invalid toContainerTree options');
  for (const key of Object.keys(options)) {
    if (key !== 'text' && !(document && key === 'roots')) throw new Error('Unknown toContainerTree option: ' + key);
  }
  if (document && options.roots !== undefined && !Array.isArray(options.roots)) throw new Error('roots must be an array');
  return options;
}
export function stateSlice(node, start, totalLength) { return {cid:node.cid, start, totalLength, items:node.value}; }
export function stateContainer(kind, cid, value) { return {type: kinds[kind], cid, value}; }
export function stateCid(peer,counter,kind) { return 'cid:'+counter+'@'+peer+':'+kinds[kind]; }
export function stateRootCid(name,kind) { return 'cid:root-'+name+':'+kinds[kind]; }
export function stateValue(value) { return {type: 'Value', value}; }
export function stateBinary(bytes) { return bytes.slice(); }
export function stateIndex(array, index, value) { Object.defineProperty(array, index, {value, enumerable: true, writable: true, configurable: true}); }
export function stateSet(object, key, value) { Object.defineProperty(object, key, {value, enumerable: true, writable: true, configurable: true}); }
