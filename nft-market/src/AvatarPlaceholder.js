import React, { useEffect, useRef } from 'react';
import { renderIcon } from './util';

const AvatarPlaceholder = ({
  address,
  alt = '',
  scale = 4,
  src,
  onClick,
  ...props
}) => {
  const imageRef = useRef(null);
  const canvasRef = useRef(null);

  useEffect(() => {
    // no src image then build it, default scale is 4
    if (!src && address) {
      const canvas = canvasRef.current;
      renderIcon({ seed: address?.toLowerCase(), scale }, canvas);
      const dataUrl = canvas?.toDataURL();
      if (dataUrl && imageRef.current) {
        imageRef.current.src = dataUrl;
      }
    }
  }, [src, scale, address]);
  return (
    <>
      <canvas ref={canvasRef} style={{ display: 'none' }} />
      <img onClick={onClick} ref={imageRef} src={src} alt={alt} {...props} />
    </>
  );
};

export default AvatarPlaceholder;
