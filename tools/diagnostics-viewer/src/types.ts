export type Pose = { x:number; y:number; z:number; yaw:number; pitch:number; roll:number };
export type Frame = Pose & { t:number; dt:number; angularVelocity:number; translationalVelocity:number; state:string; transmitted:boolean; packet?:number; rejected?:boolean };
export type Event = { t:number; type:string; label?:string };
export type Track = { formatVersion:string; metadata:Record<string,unknown>; frames:Frame[]; events:Event[] };
