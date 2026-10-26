// 用模型默认网格定位图集区域，避免把图集边缘的扩展像素当成眼睛素材。
const fs = require('fs');
const vm = require('vm');
global.__dirname = require('path').resolve('target/model-extract');
global.require = require;
vm.runInThisContext(fs.readFileSync('target/model-extract/live2dcubismcore.min.js', 'utf8'));
setTimeout(() => {
    const path = process.argv[2];
    const bytes = fs.readFileSync(path);
    const moc = Live2DCubismCore.Moc.fromArrayBuffer(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));
    const model = Live2DCubismCore.Model.fromMoc(moc);
    model.update();
    const d = model.drawables;
    const rows = Array.from(d.ids, (id, i) => ({id, index:i, texture:d.textureIndices[i], opacity:d.opacities[i], order:d.renderOrders[i], masks:Array.from(d.masks[i]), uv:Array.from(d.vertexUvs[i]), vertices:Array.from(d.vertexPositions[i]), indices:Array.from(d.indices[i])}));
    fs.writeFileSync('target/model-extract/drawables.json', JSON.stringify(rows));
    console.log(rows.length, Object.keys(d));
    model.release(); moc._release();
}, 1000);
