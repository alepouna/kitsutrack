import type { Event, Frame, Pose, Track } from './types.ts';

const MAGIC = 'KTRK';
const finite = (n:unknown, fallback=0) => typeof n === 'number' && Number.isFinite(n) ? n : fallback;
const pose = (p:Partial<Pose>):Pose => ({x:finite(p.x),y:finite(p.y),z:finite(p.z),yaw:finite(p.yaw),pitch:finite(p.pitch),roll:finite(p.roll)});
const normalize = (value:any):Track => {
  const raw = Array.isArray(value) ? {frames:value, events:[], metadata:{}} : value;
  if (!raw || !Array.isArray(raw.frames) || raw.frames.length === 0) throw new Error('No frame records were found.');
  const frames:Frame[] = raw.frames.map((f:any, i:number) => {
    const p = pose(f.output ?? f.pose ?? f);
    return {...p, t:finite(f.t, i / 60), dt:finite(f.dt, 1/60), angularVelocity:finite(f.angularVelocity ?? f.angular_velocity), translationalVelocity:finite(f.translationalVelocity ?? f.translational_velocity), state:String(f.state ?? 'tracking'), transmitted:Boolean(f.transmitted), packet:typeof f.packet === 'number' ? f.packet : undefined, rejected:Boolean(f.rejected)};
  });
  const events:Event[] = (raw.events ?? []).map((e:any) => ({t:finite(e.t), type:String(e.type ?? 'event'), label:e.label}));
  return {formatVersion:String(raw.formatVersion ?? raw.version ?? 'json-dev'), metadata:raw.metadata ?? {}, frames, events};
};
const crc32 = (bytes:Uint8Array) => { let c=~0; for(const b of bytes){c^=b; for(let k=0;k<8;k++) c=(c>>>1)^(-(c&1)&0xedb88320);} return (~c)>>>0; };
export async function decode(file:File):Promise<Track> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  const text = new TextDecoder().decode(bytes).trim();
  if (text.startsWith('{') || text.startsWith('[')) {
    try { return normalize(JSON.parse(text)); } catch {
      const lines = text.split(/\r?\n/).filter(Boolean).map(line => JSON.parse(line));
      const events = lines.filter((row:any) => row.event).map((row:any) => ({t:finite(row.t),type:String(row.event)}));
      const frames = lines.filter((row:any) => !row.event).map((row:any) => ({...row, output: quaternionPose(row.output), pose: quaternionPose(row.output), transmitted:Boolean(row.transmitted)}));
      return normalize({formatVersion:'ios-jsonl', metadata:{source:file.name}, frames, events});
    }
  }
  if (bytes.length < 25 || new TextDecoder().decode(bytes.slice(0,4)) !== MAGIC) throw new Error('Unsupported file: expected a .ktrack JSON file or KTRK binary container.');
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const version = view.getUint16(4, true); const compression=view.getUint8(6); const metadataLength=view.getUint32(7,true); const count=view.getUint32(11,true); const payloadLength=view.getUint32(15,true); const checksum=view.getUint32(19,true);
  if (version > 1) throw new Error(`Unsupported .ktrack major version ${version}.`);
  const metaStart=23, payloadStart=metaStart+metadataLength, payloadEnd=payloadStart+payloadLength;
  if (payloadEnd > bytes.length || count > 2_000_000) throw new Error('Corrupt file: declared record bounds exceed the file.');
  let metadata:Record<string,unknown>={}; try { metadata=JSON.parse(new TextDecoder().decode(bytes.slice(metaStart,payloadStart))); } catch { throw new Error('Corrupt metadata JSON.'); }
  let payload=bytes.slice(payloadStart,payloadEnd);
  if (compression===1) { if (!('DecompressionStream' in window)) throw new Error('This browser cannot decompress the .ktrack payload.'); const stream=new Blob([payload]).stream().pipeThrough(new DecompressionStream('deflate')); payload=new Uint8Array(await new Response(stream).arrayBuffer()); }
  if (crc32(payload)!==checksum) throw new Error('Checksum mismatch: the recording may be incomplete or corrupt.');
  const decoded=JSON.parse(new TextDecoder().decode(payload));
  const track=normalize({...decoded, metadata, formatVersion:`${version}.0`});
  if (track.frames.length !== count) throw new Error(`Record count mismatch: header says ${count}, payload contains ${track.frames.length}.`);
  return track;
}
function quaternionPose(q:any):Partial<Pose> {
  if (!Array.isArray(q) || q.length < 4) return {};
  const [x,y,z,w]=q.map(Number); return {yaw:Math.atan2(2*(w*y+x*z),1-2*(x*x+y*y))*180/Math.PI,pitch:Math.asin(Math.max(-1,Math.min(1,2*(w*x-y*z))))*180/Math.PI,roll:Math.atan2(2*(w*z+x*y),1-2*(x*x+z*z))*180/Math.PI};
}
export function demoTrack():Track {
  const frames:Frame[]=[]; const events:Event[]=[{t:0,type:'broadcast-start',label:'Broadcast started'},{t:3.4,type:'jump-rejected',label:'Jumpy frame rejected'},{t:5.8,type:'center',label:'Center'},{t:9.2,type:'broadcast-stop',label:'Broadcast stopped'}];
  for(let i=0;i<720;i++){const t=i/60, silent=t>=9.2; const wobble=t<9.2 ? Math.sin(t*2.2)*0.7 : 0; frames.push({t,dt:1/60,x:Math.sin(t*.8)*.035,y:Math.cos(t*.55)*.02,z:Math.sin(t*.45)*.025,yaw:Math.sin(t*.8)*8+wobble,pitch:Math.cos(t*.6)*4,roll:Math.sin(t*1.1)*3,angularVelocity:Math.abs(Math.cos(t*.8)*6),translationalVelocity:Math.abs(Math.cos(t*.8)*.04),state:silent?'tracking':'tracking',transmitted:!silent,packet:silent?undefined:i,rejected:i===204});}
  return {formatVersion:'demo',metadata:{appVersion:'0.1 debug build',deviceModel:'iPhone simulation',transportMode:'Direct UDP',settings:{rotationSmoothing:true,stationaryTimeConstantMs:180,movingTimeConstantMs:35,jumpRejection:true,recoverySampleCount:8}},frames,events};
}
